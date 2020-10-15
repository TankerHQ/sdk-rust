from typing import List, Optional
import os
import argparse
import json
import sys

import cli_ui as ui  # noqa
from path import Path
import tankerci
from tankerci.conan import TankerSource
import tankerci.conan
import tankerci.git
import tankerci.gitlab
from tankerci.build_info import DepsConfig


TARGET_LIST = [
    # "armv7-linux-androideabi",
    # "aarch64-linux-android",
    # "x86_64-linux-android",
    # "i686-linux-android",
    # "aarch64-apple-ios",
    # "x86_64-apple-ios",
    "x86_64-apple-darwin",
    # "x86_64-unknown-linux-gnu",
]


def profile_to_rust_target(platform: str, arch: str) -> str:
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
        return "x86_64-apple-darwin"
    elif platform == "iOS":
        if arch == "armv8":
            return "aarch64-apple-ios"
        elif arch == "x86_64":
            return "x86_64-apple-ios"
    elif platform == "Linux":
        return "x86_64-unknown-linux-gnu"

    raise Exception(f"Unsupported target architecture: {platform}-{arch}")


def get_android_bin_path() -> Path:
    # We need to specify an android profile or conan can't find the binary
    # package. The specific profile is not important since there is only one
    # binary NDK, the recipe ignores the arch, api_level, etc.
    tankerci.run(
        "conan",
        "install",
        "android_ndk_installer/r21d@",
        "--profile",
        "android-armv7-release",
    )
    _, out = tankerci.run_captured(
        "conan",
        "info",
        "android_ndk_installer/r21d@",
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


def bind_gen(*, header_source: Path, output_file: Path, include_path: Path) -> None:
    tankerci.run(
        "bindgen",
        "--no-layout-tests",
        header_source,
        "-o",
        output_file,
        "--",
        "-I",
        include_path,
    )


class Builder:
    def __init__(self, *, src_path: Path, tanker_source: TankerSource, profile: str):
        self.src_path = src_path
        self.profile = profile
        self.tanker_source = tanker_source
        self.platform = tankerci.conan.get_profile_key("settings.os", profile)
        self.arch = tankerci.conan.get_profile_key("settings.arch", profile)
        self.target_triplet = profile_to_rust_target(self.platform, self.arch)

    def _is_android_target(self) -> bool:
        return self.platform == "Android"

    def _is_ios_target(self) -> bool:
        return self.platform == "iOS"

    def _is_host_target(self) -> bool:
        return not (self._is_android_target() or self._is_ios_target())

    def _prepare_profile(self) -> None:
        conan_out = self.src_path / "conan" / "out" / self.profile
        package_path = conan_out / "package"
        depsConfig = DepsConfig(self.src_path / "conan" / "out" / self.profile)

        # copy includes
        package_include = package_path / "include"
        package_include.rmtree_p()
        package_include.makedirs()
        include_path = Path(depsConfig["tanker"].include_dirs[0])
        (include_path / "ctanker").merge_tree(package_include / "ctanker")
        (include_path / "ctanker.h").copy(package_include)

        # copy all .a in deplibs
        package_libs = package_path / "deplibs"
        package_libs.makedirs_p()
        for lib_path in depsConfig.all_lib_paths():
            Path(lib_path).copy(package_libs)

        native_path = self.src_path / "native" / self.target_triplet
        native_path.rmtree_p()
        native_path.makedirs_p()
        # merge all .a in deplibs into one big libtanker.a
        self._merge_all_libs(package_path, native_path)
        bind_gen(
            header_source=include_path / "ctanker.h",
            output_file=native_path / "ctanker.rs",
            include_path=include_path,
        )

    def _merge_all_libs(self, package_path: Path, native_path: Path) -> None:
        with package_path:
            env = os.environ
            if self._is_android_target():
                android_bin_path = get_android_bin_path()
                env["LD"] = android_bin_path / "ld.lld"
                env["OBJCOPY"] = android_bin_path / "llvm-objcopy"
                ui.info(f'Using {env["LD"]}')
                ui.info(f'Using {env["OBJCOPY"]}')

            if self._is_ios_target():
                env["ARMERGE_LDFLAGS"] = "-bitcode_bundle"
            Path("libtanker.a").remove_p()
            # Apple prefixes symbols with '_'
            tankerci.run(
                "armerge --keep-symbols '^_?tanker_.*' --output libtanker.a"
                " deplibs/*.a",
                shell=True,
                env=env,
            )
            if self._is_android_target():
                llvm_strip = android_bin_path / "llvm-strip"
                # HACK: Android forces debug symbols, we need to patch the
                # toolchain to remove them. Until then, strip them here.
                tankerci.run(
                    llvm_strip, "--strip-debug", "--strip-unneeded", "libtanker.a"
                )
            Path("libtanker.a").copy(native_path)

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
        if not self._is_host_target():
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
            cwd=self.src_path,
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
    for profile in profiles:
        builder = Builder(
            src_path=Path.getcwd(), tanker_source=tanker_source, profile=profile
        )
        builder.prepare(update, tanker_ref)
        # tankerci.run("cargo", "build")
        if test:
            builder.test()


def deploy(args: argparse.Namespace) -> None:
    compiled_targets = [path.basename() for path in Path("native").listdir()]
    missing_targets = [
        target for target in TARGET_LIST if target not in compiled_targets
    ]
    if missing_targets:
        ui.fatal("Aborting deploy because of missing targets:", *missing_targets)

    version = args.version
    tankerci.bump_files(version)

    Path("release").makedirs_p()

    files = ["build.rs", "Cargo.toml", "native", "src", "tests"]
    tankerci.run(
        "tar",
        "--transform",
        f"s@^@tanker-sdk-{version}/@",
        "-czf",
        f"release/tanker-sdk-{version}.tar.gz",
        *files,
    )


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
    reset_branch_parser.add_argument("branch")

    download_artifacts_parser = subparsers.add_parser("download-artifacts")
    download_artifacts_parser.add_argument("--project-id", required=True)
    download_artifacts_parser.add_argument("--pipeline-id", required=True)
    download_artifacts_parser.add_argument("--job-name", required=True)
    deploy_parser = subparsers.add_parser("deploy")
    deploy_parser.add_argument("--version", required=True)
    subparsers.add_parser("mirror")

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
            Path.getcwd(), [f"origin/{args.branch}", f"origin/{fallback}"]
        )
        tankerci.git.reset(Path.getcwd(), ref)
    elif args.command == "download-artifacts":
        tankerci.gitlab.download_artifacts(
            project_id=args.project_id,
            pipeline_id=args.pipeline_id,
            job_name=args.job_name,
        )
    elif args.command == "mirror":
        tankerci.git.mirror(github_url="git@github.com:TankerHQ/sdk-rust")
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
