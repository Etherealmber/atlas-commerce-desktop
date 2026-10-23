; The universal Windows installer, built with Inno Setup 6.3 or later.
; One setup.exe carries both the x64 (Intel/AMD) and the ARM64 binaries and
; installs the one matching the machine, so the user never has to choose an
; architecture. The release workflow compiles it in the windows-installer
; job from the two release binaries:
;
;   iscc /DVersion=0.1.0 ^
;        /DBinaryX64=...\x64\atlas-commerce-desktop.exe ^
;        /DBinaryArm64=...\arm64\atlas-commerce-desktop.exe ^
;        /DOutputDir=dist packaging\windows\atlas-commerce-desktop.iss
;
; It needs no administrator rights: the program goes to the user's own
; Programs folder with a Start menu entry, and a running copy is closed
; before an update replaces it.

#ifndef Version
  #error Version must be defined on the ISCC command line
#endif
#ifndef BinaryX64
  #error BinaryX64 must be defined on the ISCC command line
#endif
#ifndef BinaryArm64
  #error BinaryArm64 must be defined on the ISCC command line
#endif
#ifndef OutputDir
  #error OutputDir must be defined on the ISCC command line
#endif

#define AppName "Atlas Commerce Desktop"
#define AppExeName "atlas-commerce-desktop.exe"
#define AppIdentity "AtlasCommerceDesktop"

[Setup]
; Never change: this is how Windows tells an update from a new program.
AppId={{E74305A7-4960-45B6-93C9-CEA8D83B1517}
AppName={#AppName}
AppVersion={#Version}
AppVerName={#AppName} {#Version}
AppPublisher=Etherealmber
AppCopyright=© 2026 loxdpsy/Etherealmber
AppPublisherURL=https://github.com/Etherealmber/atlas-commerce-desktop
AppSupportURL=https://github.com/Etherealmber/atlas-commerce-desktop/issues
AppUpdatesURL=https://github.com/Etherealmber/atlas-commerce-desktop/releases
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
; One installer for every supported 64-bit Windows PC: x64 machines get the
; Intel/AMD binary, ARM machines the native ARM64 binary (no emulation).
; Identifiers are OR-joined with "or", matching Inno's 64BitThreeArch
; example (space-separated lists are accepted for some directives but the
; explicit form is the documented one for architecture lists).
ArchitecturesAllowed=x64compatible or arm64
ArchitecturesInstallIn64BitMode=x64compatible or arm64
MinVersion=10.0
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=atlas-commerce-desktop-v{#Version}-setup
SetupIconFile=atlas-commerce-desktop.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\{#AppExeName}
; The file version has to be numbers: a release candidate's -rc1 comes off.
#define Dash Pos("-", Version)
#if Dash > 0
  #define NumericVersion Copy(Version, 1, Dash - 1)
#else
  #define NumericVersion Version
#endif
VersionInfoVersion={#NumericVersion}.0

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer un raccourci sur le bureau"; GroupDescription: "Raccourcis supplémentaires :"; Flags: unchecked

[Files]
; Same destination name for both architectures; the Check functions decide
; which single source is installed on this machine. solidbreak separates
; the two same-named files inside the solid LZMA archive.
Source: "{#BinaryX64}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Check: IsX64; Flags: ignoreversion
Source: "{#BinaryArm64}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Check: IsARM64; Flags: ignoreversion solidbreak
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "atlas-commerce-desktop-installer.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
; atlascommerce:// links (e-mail verify, QR handoff) open in the app.
; Registered for this user only, like the program itself.
Root: HKCU; Subkey: "Software\Classes\atlascommerce"; ValueType: string; ValueName: ""; ValueData: "URL:Atlas Commerce link"
Root: HKCU; Subkey: "Software\Classes\atlascommerce"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\atlascommerce\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"",0"
Root: HKCU; Subkey: "Software\Classes\atlascommerce\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\AtlasCommerceDesktop.atlascommerce"; ValueType: string; ValueName: ""; ValueData: "URL:Atlas Commerce link"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\AtlasCommerceDesktop.atlascommerce"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\AtlasCommerceDesktop.atlascommerce\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"",0"
Root: HKCU; Subkey: "Software\Classes\AtlasCommerceDesktop.atlascommerce\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Client natif pour la plateforme Atlas Commerce"
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: """{app}\{#AppExeName}"",0"
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities"; ValueType: string; ValueName: "URLAssociations"; ValueType: "atlascommerce"; ValueData: "AtlasCommerceDesktop.atlascommerce"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppIdentity}"; ValueData: "Software\{#AppIdentity}\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Lancer {#AppName}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CloseInstalledApp;
var
  Locator, Services, Processes, Process, Path: Variant;
  Index: Integer;
  Exe: String;
begin
  Exe := ExpandConstant('{app}\{#AppExeName}');
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Services := Locator.ConnectServer('', 'root\CIMV2');
    Processes := Services.ExecQuery('SELECT * FROM Win32_Process WHERE Name = ''{#AppExeName}''');
    for Index := 0 to Processes.Count - 1 do begin
      Process := Processes.ItemIndex(Index);
      Path := Process.ExecutablePath;
      // A portable copy can have the same name. Never close it by name alone.
      if not VarIsNull(Path) then
        if CompareText(Path, Exe) = 0 then
          Log('Closing installed Atlas Commerce Desktop: result ' + IntToStr(Process.Terminate(0)));
    end;
  except
    Log('Could not close installed Atlas Commerce Desktop: ' + GetExceptionMessage);
  end;
end;

// CloseApplications covers Setup, not Uninstall. Check the installed file
// before removing anything, including the uninstaller needed to try again.
function InitializeUninstall: Boolean;
var
  Exe: String;
  Probe: TFileStream;
begin
  Result := False;
  Exe := ExpandConstant('{app}\{#AppExeName}');
  while FileExists(Exe) do begin
    try
      // Windows denies write access to a running executable. Do not write
      // anything: opening it also catches other locks on this exact copy.
      Probe := TFileStream.Create(Exe, fmOpenReadWrite or fmShareExclusive);
      Probe.Free;
      Result := True;
      Exit;
    except
      Log('Cannot open the installed executable for uninstall: ' + GetExceptionMessage);
    end;
    if UninstallSilent then
      Exit;
    case SuppressibleTaskDialogMsgBox('Atlas Commerce Desktop ne peut pas encore être supprimé.',
      'Son fichier programme est utilisé ou inaccessible. Quittez Atlas Commerce Desktop et réessayez, ou fermez-le ici pour continuer.' + #13#10#13#10 +
      'Fermer Atlas Commerce Desktop termine le processus immédiatement.',
      mbError, MB_YESNOCANCEL, ['Réessayer', 'Fermer Atlas Commerce Desktop'], 0, IDCANCEL) of
      IDYES: ;
      IDNO: begin
        CloseInstalledApp;
        Sleep(200);
      end;
    else
      Exit;
    end;
  end;
  Result := True;
end;

// The atlascommerce: scheme key is shared with whatever else opens the
// links, so uninstalling takes it away only while it still names this program.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
  Exe: String;
begin
  if CurUninstallStep <> usUninstall then
    Exit;
  if not RegQueryStringValue(HKCU, 'Software\Classes\atlascommerce\shell\open\command', '', Command) then
    Exit;
  Exe := Lowercase(ExpandConstant('{app}\{#AppExeName}'));
  if Pos(Exe, Lowercase(Command)) > 0 then
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\atlascommerce');
end;
