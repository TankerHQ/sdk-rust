import argparse
import json
import os
import shutil
import sys
from pathlib import Path
from typing import List, Optional

import cli_ui as ui  # noqa
import tankerci
import tankerci.conan
import tankerci.cpp
import tankerci.git
import tankerci.gitlab
from tankerci.build_info import DepsConfig
from tankerci.conan import Profile, TankerSource

TARGET_LIST = [
    "armv7-linux-androideabi",
    "aarch64-linux-android",
    "x86_64-linux-android",
    "i686-linux-android",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "aarch64-apple-darwin",
    "x86_64-apple-ios",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    # special one, it is the same as msvc, but the import lib is renamed to libctanker.a
    "x86_64-pc-windows-gnu",
]

NDK_ARCH_TARGETS = {
    "armv7": "arm-linux-androideabi",
    "armv8": "aarch64-linux-android",
    "x86_64": "x86_64-linux-android",
    "x86": "i686-linux-android",
}

CLANG_RT_ARCH_TARGETS = {
    "armv7": "arm-android",
    "armv8": "aarch64-android",
    "x86_64": "x86_64-android",
    "x86": "i686-android",
}


def profile_to_rust_target(platform: str, arch: str, sdk: Optional[str]) -> str:
    if platform == "Android":
        if arch == "armv7":
            return "armv7-linux-androideabi"
        elif arch == "armv8":
            return "aarch64-linux-android"
        elif arch == "x86_64":
            return "x86_64-linux-android"
        elif arch == "x86":
            return "i686-linux-android"
    elif platform == "Macos":
        if arch == "x86_64":
            return "x86_64-apple-darwin"
        elif arch == "armv8":
            return "aarch64-apple-darwin"
    elif platform == "iOS":
        if arch == "armv8":
            if sdk == "iphonesimulator":
                return "aarch64-apple-ios-sim"
            else:
                return "aarch64-apple-ios"
        elif arch == "x86_64":
            return "x86_64-apple-ios"
    elif platform == "Linux":
        return "x86_64-unknown-linux-gnu"
    elif platform == "Windows":
        return "x86_64-pc-windows-msvc"

    raise Exception(f"Unsupported target architecture: {platform}-{arch}")


def get_android_bin_path() -> Path:
    tankerci.run(
        "conan",
        "install",
        "android-ndk/r26@",
        "--profile:host",
        "linux-x86_64",
        "--profile:build",
        str(tankerci.conan.get_build_profile()),
    )
    _, out = tankerci.run_captured(
        "conan",
        "info",
        "android-ndk/r26@",
        "--profile",
        "linux-x86_64",
        "--profile:build",
        str(tankerci.conan.get_build_profile()),
        "--json",
        "--paths",
    )
    try:
        info = json.loads(out)
        package_path = Path(info[0]["package_folder"])
        bin_path = package_path / "bin/toolchains/llvm/prebuilt/linux-x86_64/bin"
        return bin_path
    except (json.JSONDecodeError, KeyError, IndexError):
        if out:
            ui.error(f"Failed to parse output: {out}")
        raise


def bind_gen(
    *, header_source: Path, output_file: Path, include_path: Path, dynamic_loading: bool
) -> None:
    args = []
    if dynamic_loading:
        args += [
            "--dynamic-loading",
            "ctanker_api",
        ]
    tankerci.run(
        "bindgen",
        *args,
        "--no-layout-tests",
        str(header_source),
        "-o",
        str(output_file),
        "--",
        "-I",
        str(include_path),
    )


class Builder:
    def __init__(
        self, *, src_path: Path, build_profile: Profile, host_profile: Profile
    ):
        self.src_path = src_path
        self.host_profile = host_profile
        self.build_profile = build_profile
        self.platform = tankerci.conan.get_profile_key(
            "settings.os", str(host_profile[0])
        )
        self.sdk = None
        if self.platform == "iOS":
            self.sdk = tankerci.conan.get_profile_key(
                "settings.os.sdk", str(host_profile[0])
            )
        self.arch = tankerci.conan.get_profile_key(
            "settings.arch", str(host_profile[0])
        )
        self.target_triplet = profile_to_rust_target(self.platform, self.arch, self.sdk)

    @property
    def _is_android_target(self) -> bool:
        return self.platform == "Android"

    @property
    def _is_ios_target(self) -> bool:
        return self.platform == "iOS"

    @property
    def _is_windows_target(self) -> bool:
        return self.platform == "Windows"

    @property
    def _is_host_target(self) -> bool:
        return not (self._is_android_target or self._is_ios_target)

    def _copy_includes(self, package_path: Path, depsConfig: DepsConfig) -> None:
        package_include = package_path / "include"
        if package_include.exists():
            shutil.rmtree(package_include)
        package_include.mkdir(parents=True)
        include_path = Path(depsConfig["tanker"].include_dirs[0])
        dest_include_path = package_include / "ctanker"
        for header in include_path.glob("**/*"):
            if header.is_dir():
                continue
            rel_dir = header.parent.relative_to(include_path)
            header_dest_dir = dest_include_path / rel_dir
            header_dest_dir.mkdir(parents=True, exist_ok=True)
            ui.info_2(header, "->", header_dest_dir)
            shutil.copy(header, header_dest_dir)

    # Import only soft float128 builtins from compiler-rt
    # This is necessary on 64bit android archs, as Clang doesn't build them by default,
    # and Google's NDK distribution doesn't take care of that either...
    @staticmethod
    def _armerge_soft_float128_compiler_rt_builtins(
        compiler_rt_lib: Path, output_path: Path, env: dict[str, str]
    ) -> None:
        f128_builtins = [
            "__addtf3",
            "__subtf3",
            "__multf3",
            "__divtf3",
            "__negtf2",
            "__extenddftf2",
            "__extendsftf2",
            "__trunctfdf2",
            "__trunctfsf2",
            "__fixdfti",
            "__fixsfti",
            "__fixunsdfti",
            "__fixunssfti",
            "__fixtfdi",
            "__fixtfsi",
            "__fixtfti",
            "__fixunstfdi",
            "__fixunstfsi",
            "__fixunstfti",
            "__floattidf",
            "__floattisf",
            "__floatuntidf",
            "__floatuntisf",
            "__floatditf",
            "__floatsitf",
            "__floattitf",
            "__floatunditf",
            "__floatunsitf",
            "__floatuntitf",
            "__cmptf2",
            "__unordtf2",
            "__eqtf2",
            "__getf2",
            "__gttf2",
            "__letf2",
            "__lttf2",
            "__netf2",
            "__powitf2",
            "__multc3",
            "__divtc3",
        ]
        keep_symbol_args = [e for sym_name in f128_builtins for e in ["-k", sym_name]]

        tankerci.run(
            "armerge",
            *keep_symbol_args,
            "--output",
            str(output_path.relative_to(Path.cwd())),
            str(compiler_rt_lib),
            shell=False,
            env=env,
        )

    def _merge_all_libs(
        self, depsConfig: DepsConfig, package_path: Path, native_path: Path
    ) -> None:
        with tankerci.working_directory(package_path):
            env = os.environ.copy()
            if self._is_android_target:
                android_bin_path = get_android_bin_path()
                env["LD"] = str(android_bin_path / "ld.lld")
                env["OBJCOPY"] = str(android_bin_path / "llvm-objcopy")
                ui.info(f'Using {env["LD"]}')
                ui.info(f'Using {env["OBJCOPY"]}')

            if self._is_ios_target:
                env["ARMERGE_LDFLAGS"] = "-bitcode_bundle"
            libctanker_a = Path("libctanker.a")
            if libctanker_a.exists():
                libctanker_a.unlink()

            cxx_package_libs = package_path / "cxx_deplibs"
            cxx_package_libs.mkdir(parents=True, exist_ok=True)
            if self._is_android_target:
                ndk_arch = NDK_ARCH_TARGETS[self.arch]
                android_lib_path = android_bin_path / f"../sysroot/usr/lib/{ndk_arch}"

                # Starting with NDK r23, Google in its infinite wisdom has decided to make things
                # more interesting
                # libgcc is gone, and now we use clang's libcxx and compiler-rt.
                # Unfortunately, the libcxx_static.a is currently missing soft float128 builtins
                # for 64bit archs (See https://reviews.llvm.org/D53608 and
                # https://github.com/llvm/llvm-project/issues/51395)
                # It is possible to find those symbols in the separate libclang_rt.builtins libs
                # However, we can't pull in all of rt.builtins, or we will have duplicate symbols
                # and fail linking
                if self.arch in ["x86_64", "armv8"]:
                    compiler_rt_arch = CLANG_RT_ARCH_TARGETS[self.arch]
                    compiler_rt_dir = android_bin_path / "../lib/clang/17/lib/linux/"
                    compiler_rt_lib = (
                        compiler_rt_dir / f"libclang_rt.builtins-{compiler_rt_arch}.a"
                    )
                    out_path = (
                        cxx_package_libs
                        / f"libclang_rt.builtins.float128-{compiler_rt_arch}.a"
                    )
                    self._armerge_soft_float128_compiler_rt_builtins(
                        compiler_rt_lib, out_path, env
                    )

                for lib in android_lib_path.glob("*.a"):
                    # Rust already links some (non-C++) NDK libs, skip to avoid duplicate symbols
                    # Also we already link rt builtins above, so we don't need/can't have these
                    skipped = [
                        "libc.a",
                        "libm.a",
                        "libdl.a",
                        "libz.a",
                        "libstdc++.a",
                        "libunwind.a",
                        "libcompiler_rt-extras.a",
                    ]
                    if lib.is_dir() or lib.name in skipped:
                        continue
                    ui.info_2(
                        "Android NDK",
                        ndk_arch,
                        "sysroot",
                        lib.name,
                        "->",
                        cxx_package_libs,
                    )
                    shutil.copy(lib, cxx_package_libs)

            package_libs = package_path / "deplibs"
            package_libs.mkdir(parents=True, exist_ok=True)
            for lib_path in depsConfig.all_lib_paths():
                ios_libcxx = ["libc++.a", "libc++abi.a"]
                if self._is_ios_target and lib_path.name in ios_libcxx:
                    dest_dir = cxx_package_libs
                else:
                    dest_dir = package_libs
                ui.info_1("copying", lib_path, "to", dest_dir)
                shutil.copy(lib_path, dest_dir)

            if self._is_android_target or self._is_ios_target:
                tankerci.run(
                    "armerge --keep-symbols '.*' --output libcxx_vendored.a cxx_deplibs/*.a",
                    shell=True,
                    env=env,
                )
                shutil.copy("libcxx_vendored.a", native_path)

            # Apple prefixes symbols with '_'
            tankerci.run(
                "armerge --keep-symbols '^_?tanker_.*' --output libctanker.a"
                " deplibs/*.a",
                shell=True,
                env=env,
            )
            if self._is_android_target:
                llvm_strip = android_bin_path / "llvm-strip"
                # HACK: Android forces debug symbols, we need to patch the
                # toolchain to remove them. Until then, strip them here.
                tankerci.run(
                    str(llvm_strip), "--strip-debug", "--strip-unneeded", "libctanker.a"
                )
            shutil.copy("libctanker.a", native_path)

    def _prepare_profile(self) -> None:
        conan_out = self.src_path / "conan" / "out" / str(self.host_profile)
        package_path = conan_out / "package"
        depsConfig = DepsConfig(
            self.src_path / "conan" / "out" / str(self.host_profile)
        )

        self._copy_includes(package_path, depsConfig)

        native_path = self.src_path / "native" / self.target_triplet
        if native_path.exists():
            shutil.rmtree(native_path)

        native_path.mkdir(parents=True)

        if self._is_windows_target:
            for lib_path in depsConfig.all_lib_paths():
                ui.info_1("copying", lib_path, "to", native_path)
                shutil.copy(lib_path, native_path)
            # handle mingw target
            mingw_path = self.src_path / "native" / "x86_64-pc-windows-gnu"
            # prepare is called twice, so ignore when dirs exists
            shutil.copytree(native_path, mingw_path, dirs_exist_ok=True)
        else:
            self._merge_all_libs(depsConfig, package_path, native_path)
        include_path = package_path / "include" / "ctanker"
        bind_gen(
            header_source=include_path / "ctanker.h",
            output_file=native_path / "ctanker.rs",
            include_path=include_path,
            dynamic_loading=self._is_windows_target,
        )
        if self._is_windows_target:
            shutil.copyfile(native_path / "ctanker.rs", mingw_path / "ctanker.rs")

    def prepare(
        self,
        update: bool,
        tanker_source: TankerSource,
        tanker_ref: Optional[str] = None,
    ) -> None:
        tanker_deployed_ref = tanker_ref
        if tanker_source == TankerSource.DEPLOYED and not tanker_ref:
            tanker_deployed_ref = "tanker/latest-stable@"
        tankerci.conan.install_tanker_source(
            tanker_source,
            output_path=Path("conan") / "out",
            host_profiles=[self.host_profile],
            build_profile=self.build_profile,
            update=update,
            tanker_deployed_ref=tanker_deployed_ref,
        )
        self._prepare_profile()

    def _cargo(self, subcommand: str, *extra_args: str) -> None:
        env = os.environ.copy()
        if self._is_android_target:
            android_bin_path = get_android_bin_path()
            env["CC"] = str(android_bin_path / "clang")
            env["AR"] = str(android_bin_path / "llvm-ar")
            ui.info(f'Using {env["CC"]}')
            ui.info(f'Using {env["AR"]}')

        tankerci.run(
            "cargo",
            subcommand,
            "--target",
            self.target_triplet,
            *extra_args,
            cwd=self.src_path,
            env=env,
        )
        if self._is_windows_target:
            tankerci.run(
                "cargo",
                subcommand,
                "--target",
                "x86_64-pc-windows-gnu",
                *extra_args,
                cwd=self.src_path,
            )

    def build(self) -> None:
        if not self._is_host_target:
            if self.target_triplet == "aarch64-apple-ios-sim":
                tankerci.run(
                    "cargo",
                    "+nightly",
                    "build",
                    "-Z",
                    "build-std",
                    "--target",
                    self.target_triplet,
                    cwd=self.src_path,
                )
                return
        self._cargo("build")

    def test(self) -> None:
        self.build()

        if not self._is_host_target:
            ui.info(
                str(self.host_profile), "is a cross-compiled target, skipping tests"
            )
            return

        tankerci.run("cargo", "fmt", "--", "--check", cwd=self.src_path)
        tankerci.run(
            "cargo",
            "clippy",
            "--all-targets",
            "--",
            "--deny",
            "warnings",
            "--allow",
            "unknown-lints",
            cwd=self.src_path,
        )
        if self._is_windows_target:
            shutil.copy(
                Path("native") / self.target_triplet / "ctanker.dll",
                Path("target") / "debug/deps",
            )
        self._cargo("test")
        self._cargo(
            "test", "--no-default-features"
        )  # Also test without HTTP reverse bindings on desktops


def prepare(
    tanker_source: TankerSource,
    *,
    profiles: List[Profile],
    update: bool = False,
    tanker_ref: Optional[str] = None,
) -> None:
    build_profile = tankerci.conan.get_build_profile()
    for host_profile in profiles:
        builder = Builder(
            src_path=Path.cwd(), host_profile=host_profile, build_profile=build_profile
        )
        builder.prepare(update, tanker_source, tanker_ref)


def build(
    *,
    profiles: List[Profile],
    test: bool = False,
) -> None:
    build_profile = tankerci.conan.get_build_profile()
    if os.environ.get("CI"):
        os.environ["RUSTFLAGS"] = "-D warnings"
    for host_profile in profiles:
        builder = Builder(
            src_path=Path.cwd(), host_profile=host_profile, build_profile=build_profile
        )
        # build is implied with test
        if test:
            builder.test()
        else:
            builder.build()


def deploy(args: argparse.Namespace) -> None:
    compiled_targets = [p.name for p in Path("native").iterdir() if p.is_dir()]
    missing_targets = [
        target for target in TARGET_LIST if target not in compiled_targets
    ]
    if missing_targets:
        ui.fatal("Aborting deploy because of missing targets:", *missing_targets)

    version = args.version
    registry = args.registry
    tankerci.bump_files(version)

    tankerci.run("cargo", "publish", "--allow-dirty", f"--registry={registry}")


def matching_downstream_branch(repo: str) -> str:
    current_ref = os.environ.get(
        "UPSTREAM_COMMIT_REF_NAME", os.environ["CI_COMMIT_REF_NAME"]
    )
    if tankerci.git.remote_branch_exists(current_ref, repo):
        return current_ref
    else:
        return "master"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--isolate-conan-user-home",
        action="store_true",
        dest="home_isolation",
        default=False,
    )
    parser.add_argument("--remote", default="artifactory")
    subparsers = parser.add_subparsers(title="subcommands", dest="command")

    build_parser = subparsers.add_parser("build")
    build_parser.add_argument(
        "--profile",
        dest="profiles",
        action="append",
        required=True,
        nargs="+",
        type=str,
    )
    build_parser.add_argument("--test", action="store_true")

    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument(
        "--profile",
        dest="profiles",
        action="append",
        required=True,
        nargs="+",
        type=str,
    )
    prepare_parser.add_argument("--tanker-ref")
    prepare_parser.add_argument(
        "--use-tanker",
        type=TankerSource,
        default=TankerSource.EDITABLE,
        dest="tanker_source",
    )
    prepare_parser.add_argument(
        "--update", action="store_true", default=False, dest="update"
    )

    download_artifacts_parser = subparsers.add_parser("download-artifacts")
    download_artifacts_parser.add_argument("--project-id", required=True)
    download_artifacts_parser.add_argument("--pipeline-id", required=True)
    download_artifacts_parser.add_argument("--job-name", required=True)

    deploy_parser = subparsers.add_parser("deploy")
    deploy_parser.add_argument("--version", required=True)
    deploy_parser.add_argument("--registry", required=True)

    write_bridge_dotenv = subparsers.add_parser("write-bridge-dotenv")
    write_bridge_dotenv.add_argument(
        "--downstream", dest="downstreams", action="append", required=True
    )

    args = parser.parse_args()
    user_home = None
    if args.home_isolation:
        user_home = Path.cwd() / ".cache" / "conan" / args.remote

    if args.command == "build":
        profiles = [Profile(p) for p in args.profiles]
        with tankerci.conan.ConanContextManager(
            [args.remote, "conancenter"], conan_home=user_home
        ):
            build(
                profiles=profiles,
                test=args.test,
            )
    elif args.command == "deploy":
        deploy(args)
    elif args.command == "prepare":
        with tankerci.conan.ConanContextManager(
            [args.remote, "conancenter"], conan_home=user_home
        ):
            profiles = [Profile(p) for p in args.profiles]
            prepare(
                args.tanker_source,
                profiles=profiles,
                update=args.update,
                tanker_ref=args.tanker_ref,
            )
    elif args.command == "download-artifacts":
        tankerci.gitlab.download_artifacts(
            project_id=args.project_id,
            pipeline_id=args.pipeline_id,
            job_name=args.job_name,
        )
    elif args.command == "write-bridge-dotenv":
        branches = [matching_downstream_branch(repo) for repo in args.downstreams]
        keys = [
            repo.replace("-", "_").upper() + "_BRIDGE_BRANCH"
            for repo in args.downstreams
        ]
        env_list = "\n".join([f"{k}={v}" for k, v in zip(keys, branches)])
        with open("bridge.env", "a+") as f:
            f.write(env_list)
        ui.info(env_list)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
