from typing import List, Optional
import os
import argparse
import json
from pathlib import Path
import shutil
import sys

import cli_ui as ui  # noqa
import tankerci
from tankerci.conan import TankerSource
import tankerci.conan
import tankerci.git
import tankerci.gitlab
import tankerci.cpp
from tankerci.build_info import DepsConfig


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
            # TODO this is Tier 3, wait for a few weeks before being able to build for armv8 simulator
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
    # We need to specify an android profile or conan can't find the binary
    # package. The specific profile is not important since there is only one
    # binary NDK, the recipe ignores the arch, api_level, etc.
    tankerci.run(
        "conan",
        "install",
        "android_ndk_installer/r22b@",
        "--profile",
        "android-armv7-release",
    )
    _, out = tankerci.run_captured(
        "conan",
        "info",
        "android_ndk_installer/r22b@",
        "--profile",
        "android-armv7-release",
        "--json",
        "--paths",
    )
    try:
        info = json.loads(out)
        package_path = Path(info[0]["package_folder"])
        bin_path = package_path / "toolchains/llvm/prebuilt/linux-x86_64/bin"
        return bin_path
    except (json.JSONDecodeError, KeyError, IndexError):
        if out:
            ui.error(f"Failed to parse output: {out}")
        raise


def bind_gen(
    *, header_source: Path, output_file: Path, include_path: Path, dynamic_loading: bool
) -> None:
    # bindgen will call clang, which needs vcvarsall to be set
    # otherwise, it will fail to find stdbool.h
    tankerci.cpp.set_build_env()
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
    def __init__(self, *, src_path: Path, tanker_source: TankerSource, profile: str):
        self.src_path = src_path
        self.profile = profile
        self.tanker_source = tanker_source
        self.platform = tankerci.conan.get_profile_key("settings.os", profile)
        self.sdk = None
        if self.platform == "iOS":
            self.sdk = tankerci.conan.get_profile_key("settings.os.sdk", profile)
        self.arch = tankerci.conan.get_profile_key("settings.arch", profile)
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

            package_libs = package_path / "deplibs"
            package_libs.mkdir(parents=True, exist_ok=True)
            for lib_path in depsConfig.all_lib_paths():
                ui.info_1("copying", lib_path, "to", package_libs)
                shutil.copy(lib_path, package_libs)

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
        conan_out = self.src_path / "conan" / "out" / self.profile
        package_path = conan_out / "package"
        depsConfig = DepsConfig(self.src_path / "conan" / "out" / self.profile)

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
            if mingw_path.exists():
                shutil.rmtree(mingw_path)
            shutil.copytree(native_path, mingw_path)
            # rename import lib to what GCC expects
            os.rename(mingw_path / "ctanker.lib", mingw_path / "libctanker.a")
            os.rename(
                mingw_path / "tanker_admin-c.lib", mingw_path / "libtanker_admin-c.a"
            )
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
            shutil.copy(
                native_path / "ctanker.rs",
                self.src_path / "native" / "x86_64-pc-windows-gnu" / "ctanker.rs",
            )

    def prepare(self, update: bool, tanker_ref: Optional[str] = None) -> None:
        tanker_deployed_ref = tanker_ref
        if self.tanker_source == TankerSource.DEPLOYED and not tanker_ref:
            tanker_deployed_ref = "tanker/latest-stable@"
        tankerci.conan.install_tanker_source(
            self.tanker_source,
            output_path=Path("conan") / "out",
            profiles=[self.profile],
            update=update,
            tanker_deployed_ref=tanker_deployed_ref,
        )
        self._prepare_profile()

    def test(self) -> None:
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
            else:
                tankerci.run(
                    "cargo", "build", "--target", self.target_triplet, cwd=self.src_path
                )
            ui.info(self.profile, "is a cross-compiled target, skipping tests")
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
        tankerci.run(
            "cargo", "test", "--target", self.target_triplet, cwd=self.src_path
        )


def build_and_test(
    tanker_source: TankerSource,
    profiles: List[str],
    *,
    update: bool = False,
    test: bool = True,
    tanker_ref: Optional[str] = None,
) -> None:
    if os.environ.get("CI"):
        os.environ["RUSTFLAGS"] = "-D warnings"
    for profile in profiles:
        builder = Builder(
            src_path=Path.cwd(), tanker_source=tanker_source, profile=profile
        )
        builder.prepare(update, tanker_ref)
        if test:
            builder.test()


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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--isolate-conan-user-home",
        action="store_true",
        dest="home_isolation",
        default=False,
    )
    subparsers = parser.add_subparsers(title="subcommands", dest="command")

    build_parser = subparsers.add_parser("build-and-test")
    build_parser.add_argument(
        "--profile", dest="profiles", action="append", required=True
    )
    build_parser.add_argument("--tanker-ref")
    build_parser.add_argument(
        "--use-tanker",
        type=TankerSource,
        default=TankerSource.EDITABLE,
        dest="tanker_source",
    )
    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument(
        "--profile", dest="profiles", action="append", required=True
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

    reset_branch_parser = subparsers.add_parser("reset-branch")
    reset_branch_parser.add_argument("branch", nargs="?")

    download_artifacts_parser = subparsers.add_parser("download-artifacts")
    download_artifacts_parser.add_argument("--project-id", required=True)
    download_artifacts_parser.add_argument("--pipeline-id", required=True)
    download_artifacts_parser.add_argument("--job-name", required=True)
    deploy_parser = subparsers.add_parser("deploy")
    deploy_parser.add_argument("--version", required=True)
    deploy_parser.add_argument("--registry", required=True)

    args = parser.parse_args()
    if args.home_isolation:
        tankerci.conan.set_home_isolation()
        tankerci.conan.update_config()

    if args.command == "build-and-test":
        build_and_test(args.tanker_source, args.profiles, tanker_ref=args.tanker_ref)
    elif args.command == "deploy":
        deploy(args)
    elif args.command == "prepare":
        build_and_test(
            args.tanker_source,
            args.profiles,
            test=False,
            update=args.update,
            tanker_ref=args.tanker_ref,
        )
    elif args.command == "reset-branch":
        fallback = os.environ["CI_COMMIT_REF_NAME"]
        ref = tankerci.git.find_ref(
            Path.cwd(), [f"origin/{args.branch}", f"origin/{fallback}"]
        )
        tankerci.git.reset(Path.cwd(), ref, clean=False)
    elif args.command == "download-artifacts":
        tankerci.gitlab.download_artifacts(
            project_id=args.project_id,
            pipeline_id=args.pipeline_id,
            job_name=args.job_name,
        )
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
