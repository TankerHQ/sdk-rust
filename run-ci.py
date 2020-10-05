from typing import Any, List, Iterator
from dataclasses import dataclass
import os
import argparse
import json
import sys
import platform
from enum import Enum

import attr
import cli_ui as ui  # noqa
from path import Path
import tankerci
import tankerci.conan
import tankerci.git


DEPLOYED_TANKER = "tanker/2.6.2@tanker/stable"
LOCAL_TANKER = "tanker/dev@"


class TankerSource(Enum):
    LOCAL = "local"
    SAME_AS_BRANCH = "same-as-branch"
    DEPLOYED = "deployed"


def get_lib_name(name: str) -> str:
    if sys.platform == "win32":
        return name + ".lib"
    else:
        return "lib" + name + ".a"


def find_libs(names: List[str], paths: List[str]) -> Iterator[Path]:
    for name in names:
        found = False
        lib_name = get_lib_name(name)
        for lib_path in paths:
            candidate = Path(lib_path) / lib_name
            if candidate.exists():
                found = True
                ui.info(f"Adding lib {candidate}")
                yield candidate
        if not found:
            if lib_name == "libctanker.a":
                raise RuntimeError(
                    f"{lib_name} was not found in {paths}, are you sure you are using"
                    " the correct profile?"
                )
            else:
                # This is not an error because some libs link with pthread, and it's
                # fine if we don't find it, we'll take it from the target system.
                ui.warning(f"Skipping {lib_name}, not found in {paths}")


TARGET_LIST = [
    "armv7-linux-androideabi",
    "aarch64-linux-android",
    "x86_64-linux-android",
    "i686-linux-android",
    "aarch64-apple-ios",
    "x86_64-apple-ios",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
]


def profile_to_rust_target(profile: str) -> str:
    split_profile = profile.split("-")
    if len(split_profile) >= 2:
        (platform, arch, *_) = split_profile
    else:
        (platform, arch) = (profile, "")

    if platform == "android":
        if arch == "armv7":
            return "armv7-linux-androideabi"
        elif arch == "armv8":
            return "aarch64-linux-android"
        elif arch == "x86_64":
            return "x86_64-linux-android"
        elif arch == "x86":
            return "i686-linux-android"
    elif platform == "macos":
        return "x86_64-apple-darwin"
    elif platform == "ios":
        if arch == "armv8":
            return "aarch64-apple-ios"
        elif arch == "x86_64":
            return "x86_64-apple-ios"
    elif platform.startswith("gcc") or platform.startswith("clang"):
        return "x86_64-unknown-linux-gnu"

    raise Exception("Unsupported target architecture: " + profile)


def is_host_target(profile: str) -> bool:
    return profile_to_rust_target(profile) in [
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
    ]


def is_android_target(profile: str) -> bool:
    return profile.startswith("android-")


def is_ios_target(profile: str) -> bool:
    return profile.startswith("ios-")


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


@attr.s(frozen=True)
class BuildConfig:
    include_path: Path = attr.ib()
    build_path: Path = attr.ib()
    libs: List[Path] = attr.ib()


class Builder:
    def __init__(self, *, src_path: Path, tanker_source: TankerSource, profile: str):
        self.src_path = src_path
        self.profile = profile

        if tanker_source in [TankerSource.LOCAL, TankerSource.SAME_AS_BRANCH]:
            self.tanker_conan_ref = LOCAL_TANKER
            self.tanker_conan_extra_flags = ["--build=tanker"]
        elif tanker_source == TankerSource.DEPLOYED:
            self.tanker_conan_ref = DEPLOYED_TANKER
            self.tanker_conan_extra_flags = []

        self.target_triple = profile_to_rust_target(self.profile)
        self.native_path = self.src_path / "native" / self.target_triple
        self.native_path.rmtree_p()
        self.native_path.makedirs()
        self.package_path = self.src_path / "package" / self.target_triple
        self.package_path.rmtree_p()
        self.package_path.makedirs()

        assert self.target_triple in TARGET_LIST

    def install_sdk_native(self) -> None:
        # fmt: off
        tankerci.conan.run(
            "install", self.tanker_conan_ref,
            *self.tanker_conan_extra_flags,
            "--update",
            "--profile", self.profile,
            "--install-folder", self.native_path,
            "--generator", "json",
        )
        # fmt: on

    def get_build_config(self) -> BuildConfig:
        build_info = self.native_path / "conanbuildinfo.json"
        conaninfo = json.loads(build_info.text())
        libs: List[str] = []
        for dep_info in conaninfo["dependencies"]:
            libs_for_dep = dep_info["libs"]
            lib_paths = dep_info["lib_paths"]
            libs.extend(find_libs(libs_for_dep, lib_paths))

        all_deps = conaninfo["dependencies"]
        tanker_packages = [x for x in all_deps if x["name"] == "tanker"]

        n = len(tanker_packages)
        assert n == 1, "expecting one package named 'tanker', got %i" % n
        tanker_package = tanker_packages[0]
        tanker_include_paths = tanker_package["include_paths"]
        n = len(tanker_include_paths)
        assert n == 1, "expecting include_path  for 'tanker', got %i" % n
        include_path = Path(tanker_package["include_paths"][0])
        build_path = Path(tanker_package["build_paths"][0])

        return BuildConfig(include_path=include_path, build_path=build_path, libs=libs)

    def build(self) -> None:
        build_config = self.get_build_config()
        libs = build_config.libs
        include_path = Path(build_config.include_path)
        stdcpplibs_path = Path(build_config.build_path) / "libstdcpp"

        # Prepare ./native/ folder for consumption by build.rs
        # copy includes
        package_include = self.package_path / "include"
        package_include.rmtree_p()
        package_include.makedirs()
        (include_path / "ctanker").merge_tree(package_include / "ctanker")
        (include_path / "ctanker.h").copy(package_include)

        # copy all .a in deplibs
        package_libs = self.package_path / "deplibs"
        package_libs.makedirs_p()
        for lib in libs:
            Path(lib).copy(package_libs)
        if stdcpplibs_path.exists():
            for stdcpplib in stdcpplibs_path.files("*.a"):
                stdcpplib.copy(package_libs)

        env = os.environ
        if is_android_target(self.profile):
            android_bin_path = get_android_bin_path()
            env["LD"] = android_bin_path / "ld.lld"
            env["OBJCOPY"] = android_bin_path / "llvm-objcopy"
            ui.info(f'Using {env["LD"]}')
            ui.info(f'Using {env["OBJCOPY"]}')

        if is_ios_target(self.profile):
            env["ARMERGE_LDFLAGS"] = "-bitcode_bundle"

        # merge all .a in deplibs into one big libtanker.a
        with self.package_path:
            if platform.system() in ["Linux", "Darwin"]:
                Path("libtanker.a").remove_p()

                # Apple prefixes symbols with '_'
                tankerci.run(
                    "armerge --keep-symbols '^_?tanker_.*' --output libtanker.a"
                    " deplibs/*.a",
                    shell=True,
                    env=env,
                )

                if is_android_target(self.profile):
                    llvm_strip = android_bin_path / "llvm-strip"
                    # HACK: Android forces debug symbols, we need to patch the
                    # toolchain to remove them. Until then, strip them here.
                    tankerci.run(
                        llvm_strip, "--strip-debug", "--strip-unneeded", "libtanker.a"
                    )
            else:
                raise RuntimeError(f"unsupported platform {platform.system()}")
            Path("libtanker.a").copy(self.native_path)

        # fmt: off
        tankerci.run(
            "bindgen", "--no-layout-tests",
            include_path / "ctanker.h",
            "-o", self.native_path / "ctanker.rs",
            "--",
            "-I", include_path,
        )
        tankerci.run(
            "bindgen", "--no-layout-tests",
            include_path / "ctanker" / "admin.h",
            "-o", self.native_path / "cadmin.rs",
            "--",
            "-I", include_path,
        )
        # fmt: on

    def test(self) -> None:
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
        tankerci.run("cargo", "test", "--target", self.target_triple, cwd=self.src_path)


@dataclass
class TreeConfig:
    src_path: Path
    tanker_source: Path


def setup_tree(args: Any) -> TreeConfig:
    src_path = Path.getcwd()

    if args.tanker_source == TankerSource.LOCAL:
        tankerci.conan.export(src_path=Path.getcwd().parent / "sdk-native")
    elif args.tanker_source == TankerSource.SAME_AS_BRANCH:
        workspace = tankerci.git.prepare_sources(repos=["sdk-native", "sdk-rust"])
        src_path = workspace / "sdk-rust"
        tankerci.conan.export(src_path=workspace / "sdk-native")

    return TreeConfig(src_path=src_path, tanker_source=args.tanker_source)


def check(args: argparse.Namespace) -> None:
    tree_config = setup_tree(args)

    for profile in args.profiles:
        builder = Builder(
            src_path=tree_config.src_path,
            tanker_source=tree_config.tanker_source,
            profile=profile,
        )
        builder.install_sdk_native()
        builder.build()
        if is_host_target(profile):
            builder.test()
        else:
            ui.info(profile, "is a cross-compiled target, skipping tests")


def deploy(args: argparse.Namespace) -> None:
    compiled_targets = [path.basename() for path in Path("native").listdir()]
    missing_targets = [
        target for target in TARGET_LIST if target not in compiled_targets
    ]
    if missing_targets:
        ui.fatal("Aborting deploy because of missing targets:", *missing_targets)

    git_tag = os.environ["CI_COMMIT_TAG"]
    version = tankerci.version_from_git_tag(git_tag)
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

    build_parser = subparsers.add_parser("build-and-check")
    build_parser.add_argument(
        "--profile", dest="profiles", action="append", required=True
    )
    build_parser.add_argument(
        "--use-tanker",
        type=TankerSource,
        default=TankerSource.LOCAL,
        dest="tanker_source",
    )

    subparsers.add_parser("deploy")
    subparsers.add_parser("mirror")

    args = parser.parse_args()
    if args.home_isolation:
        tankerci.conan.set_home_isolation()
        tankerci.conan.update_config()

    if args.command == "build-and-check":
        check(args)
    elif args.command == "deploy":
        deploy(args)
    elif args.command == "mirror":
        tankerci.git.mirror(github_url="git@github.com:TankerHQ/sdk-rust")
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
