#!/usr/bin/env python3
"""Build the Windows Installer package from a release payload.

    python packaging/windows/build-msi.py PAYLOAD PACKAGE VERSION ARCH

PAYLOAD is a folder holding atlas-commerce-desktop.exe, README.md, LICENSE
and atlas-commerce-desktop-installer.txt; PACKAGE is the .msi to write;
VERSION is the release (0.1.0, or 0.1.0-rc1); ARCH is amd64 or arm64.
native-packages runs it this way for the windows-* targets in
native-packages.yaml.

Needs Windows and the WiX Toolset 5 command line
(dotnet tool install --global wix --version 5.0.2). The package is a plain
per-user MSI with no custom actions, so no C compiler is required. It writes
nothing beside PACKAGE: native-packages refuses any other output.
"""

import argparse
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import uuid

WIX = "5.0.2"
# native-packages architecture: WiX -arch value and PE machine type.
ARCHES = {"amd64": ("x64", 0x8664), "arm64": ("arm64", 0xAA64)}
PAYLOAD = (
    "atlas-commerce-desktop.exe",
    "README.md",
    "LICENSE",
    "atlas-commerce-desktop-installer.txt",
)
# Fixed product identity: never regenerate, or Windows would treat every
# build as a different product instead of an upgrade.
UPGRADE_CODE = "07D6D2FA-2A3F-42BE-8938-B2AFC2A17C7B"


def stable_guid(name: str) -> str:
    """A deterministic GUID per component, so builds are reproducible."""
    return "{" + str(uuid.uuid5(uuid.NAMESPACE_URL, f"atlas-commerce-desktop:{name}")).upper() + "}"


def numeric_version(version: str) -> str:
    """Windows Installer compares three numbers: 0.1.0-rc1 ships as 0.1.0."""
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?", version)
    if not match:
        raise ValueError(f"Expected a release version such as 0.1.0, not {version!r}")
    major, minor, patch = (int(part) for part in match.groups())
    if major > 255 or minor > 255 or patch > 65535:
        raise ValueError(f"Windows Installer cannot compare version {version}")
    return f"{major}.{minor}.{patch}"


def pe_machine(path: Path) -> int:
    """The machine type in a Windows executable's PE header."""
    with open(path, "rb") as stream:
        dos = stream.read(64)
        if len(dos) < 64 or dos[:2] != b"MZ":
            raise ValueError(f"{path} is not a Windows executable")
        stream.seek(struct.unpack_from("<I", dos, 0x3C)[0])
        header = stream.read(6)
    if len(header) < 6 or header[:4] != b"PE\0\0":
        raise ValueError(f"{path} has no PE header")
    return struct.unpack_from("<H", header, 4)[0]


def run(command, cwd=None):
    """Runs a tool and shows its output; a failure ends the build with it."""
    try:
        result = subprocess.run(
            [str(part) for part in command], cwd=cwd, capture_output=True,
            text=True, errors="replace",
        )
    except FileNotFoundError:
        raise SystemExit(
            f"{command[0]} is not on PATH; install WiX {WIX} (dotnet tool install --global wix)"
        ) from None
    sys.stdout.write(result.stdout)
    if result.returncode:
        sys.stderr.write(result.stdout + result.stderr)
        raise SystemExit(f"{Path(command[0]).name} failed with exit code {result.returncode}")


def wxs(payload: Path, version: str) -> str:
    """The WiX v5 source: per-user files, a Start menu shortcut and the
    atlascommerce:// URL protocol, all under HKCU."""
    files = []
    for name in PAYLOAD:
        # One component per file, each with its own key path: the classic
        # per-user layout WiX can upgrade and repair without a registry
        # marker in every component.
        files.append(f"""
        <Component Id="File{len(files)}" Guid="{stable_guid(name)}">
          <File Id="File{len(files)}" Name="{name}" Source="{payload / name}" KeyPath="yes" />
        </Component>""")

    # WiX v5 layout: the Directory tree only nests Directory elements,
    # while ComponentGroup entries are Package-level siblings bound to a
    # directory through their Directory attribute (WIX0005 otherwise).
    #
    # The directory Id is the PUBLIC property INSTALLFOLDER on purpose:
    # fastframe-update's helper runs `msiexec ... INSTALLFOLDER="<running
    # exe folder>"` to upgrade the app exactly where it was installed. A
    # mixed-case (private) Id such as AppFolder cannot be overridden by a
    # command-line property, so the MSI used to land in its own default
    # folder while the helper relaunched the untouched old executable —
    # the app closed and reopened on the same version ("flicker").
    return f"""<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="Atlas Commerce Desktop"
           Manufacturer="Etherealmber"
           Version="{version}"
           UpgradeCode="{UPGRADE_CODE}"
           Scope="perUser"
           Compressed="yes">
    <MajorUpgrade DowngradeErrorMessage="Une version plus récente d'Atlas Commerce Desktop est déjà installée." />
    <MediaTemplate EmbedCab="yes" />

    <StandardDirectory Id="LocalAppDataFolder">
      <Directory Id="INSTALLFOLDER" Name="Atlas Commerce Desktop" />
    </StandardDirectory>
    <StandardDirectory Id="ProgramMenuFolder">
      <Directory Id="ProgramMenuDir" Name="Atlas Commerce Desktop" />
    </StandardDirectory>

    <ComponentGroup Id="AppFiles" Directory="INSTALLFOLDER">{''.join(files)}
    </ComponentGroup>

    <ComponentGroup Id="Shortcuts" Directory="ProgramMenuDir">
      <Component Id="StartMenuShortcut" Guid="{stable_guid('start-menu')}">
        <Shortcut Id="StartMenuShortcut" Name="Atlas Commerce Desktop"
                  Target="[INSTALLFOLDER]atlas-commerce-desktop.exe"
                  WorkingDirectory="INSTALLFOLDER" Advertise="no" />
        <RegistryValue Root="HKMU" Key="Software\\Etherealmber\\AtlasCommerceDesktop"
                       Name="StartMenu" Type="string" Value="1" KeyPath="yes" />
      </Component>
    </ComponentGroup>

    <ComponentGroup Id="Protocol" Directory="INSTALLFOLDER">
      <Component Id="UrlProtocol" Guid="{stable_guid('protocol')}">
        <RegistryValue Root="HKMU" Key="Software\\Classes\\atlascommerce"
                       Type="string" Value="URL:Atlas Commerce link" KeyPath="yes" />
        <RegistryValue Root="HKMU" Key="Software\\Classes\\atlascommerce"
                       Name="URL Protocol" Type="string" Value="" />
        <RegistryValue Root="HKMU" Key="Software\\Classes\\atlascommerce\\DefaultIcon"
                       Type="string" Value='"[INSTALLFOLDER]atlas-commerce-desktop.exe",0' />
        <RegistryValue Root="HKMU" Key="Software\\Classes\\atlascommerce\\shell\\open\\command"
                       Type="string" Value='"[INSTALLFOLDER]atlas-commerce-desktop.exe" "%1"' />
      </Component>
      <Component Id="RegisteredApplication" Guid="{stable_guid('registered-app')}">
        <RegistryValue Root="HKMU" Key="Software\\RegisteredApplications"
                       Name="AtlasCommerceDesktop" Type="string"
                       Value="Software\\Etherealmber\\AtlasCommerceDesktop\\Capabilities" KeyPath="yes" />
        <RegistryValue Root="HKMU" Key="Software\\Etherealmber\\AtlasCommerceDesktop\\Capabilities"
                       Name="ApplicationName" Type="string" Value="Atlas Commerce Desktop" />
        <RegistryValue Root="HKMU" Key="Software\\Etherealmber\\AtlasCommerceDesktop\\Capabilities"
                       Name="ApplicationDescription" Type="string"
                       Value="Client natif pour la plateforme Atlas Commerce" />
        <RegistryValue Root="HKMU" Key="Software\\Etherealmber\\AtlasCommerceDesktop\\Capabilities\\URLAssociations"
                       Name="atlascommerce" Type="string" Value="AtlasCommerceDesktop.atlascommerce" />
      </Component>
    </ComponentGroup>

    <Feature Id="Main" Title="Atlas Commerce Desktop" Level="1">
      <ComponentGroupRef Id="AppFiles" />
      <ComponentGroupRef Id="Shortcuts" />
      <ComponentGroupRef Id="Protocol" />
    </Feature>
  </Package>
</Wix>
"""


def build(payload: Path, package: Path, version: str, arch: str) -> None:
    payload = payload.resolve()
    package = package.resolve()
    wix_arch, machine = ARCHES[arch]
    numeric = numeric_version(version)
    for name in PAYLOAD:
        if not (payload / name).is_file():
            raise SystemExit(f"{payload} has no {name}")
    executable = payload / "atlas-commerce-desktop.exe"
    if pe_machine(executable) != machine:
        raise SystemExit(f"{executable} is not built for {arch}")

    with tempfile.TemporaryDirectory(prefix="atlas-msi-") as directory:
        work = Path(directory)
        source = work / "atlas-commerce-desktop.wxs"
        source.write_text(wxs(payload, numeric), encoding="utf-8")
        run([
            "wix", "build", source, "-arch", wix_arch,
            "-intermediatefolder", work / "obj",
            "-pdbtype", "none",
            "-o", package,
        ], cwd=work)
    if not package.is_file():
        raise SystemExit(f"WiX did not write {package}")


def main(argv=None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("payload", type=Path)
    parser.add_argument("package", type=Path)
    parser.add_argument("version")
    parser.add_argument("arch", choices=ARCHES)
    args = parser.parse_args(argv)
    build(args.payload, args.package, args.version, args.arch)


if __name__ == "__main__":
    main()
