; The Windows installer, built with Inno Setup 6.3 or later from a release
; binary (the release workflow does this on every tag):
;
;   iscc /DVersion=0.1.0 /DArch=x86_64 /DBinary=...\atlas-commerce-desktop.exe ^
;        /DOutputDir=dist packaging\windows\atlas-commerce-desktop.iss
;
; Arch is x86_64 or aarch64, as in the Rust target triple, so the installer
; is named like the zip next to it. It needs no administrator rights: the
; program goes to the user's own Programs folder with a Start menu entry,
; and a running copy is closed before an update replaces it.

#ifndef Version
  #error Version must be defined on the ISCC command line
#endif
#ifndef Arch
  #error Arch must be defined on the ISCC command line (x86_64 or aarch64)
#endif
#ifndef Binary
  #error Binary must be defined on the ISCC command line
#endif
#ifndef OutputDir
  #error OutputDir must be defined on the ISCC command line
#endif
#if Arch == "aarch64"
  #define InnoArch "arm64"
#else
  #define InnoArch "x64compatible"
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
ArchitecturesAllowed={#InnoArch}
ArchitecturesInstallIn64BitMode={#InnoArch}
MinVersion=10.0
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=atlas-commerce-desktop-v{#Version}-{#Arch}-pc-windows-msvc-setup
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
Source: "{#Binary}"; DestDir: "{app}"; Flags: ignoreversion
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
Root: HKCU; Subkey: "Software\{#AppIdentity}\Capabilities\URLAssociations"; ValueType: string; ValueName: "atlascommerce"; ValueData: "AtlasCommerceDesktop.atlascommerce"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppIdentity}"; ValueData: "Software\{#AppIdentity}\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Lancer {#AppName}"; Flags: nowait postinstall skipifsilent

[Code]
#if Arch == "x86_64"
// x64compatible lets this installer run on an ARM PC under emulation, and a
// copy installed from it keeps updating to x64. The ARM installer runs
// natively, so point to it. A silent install (an update) goes ahead.
function InitializeSetup(): Boolean;
begin
  Result := True;
  if IsArm64 and not WizardSilent then
    Result := MsgBox('Ce PC utilise un processeur ARM. Le programme d''installation ' +
      'ARM (aarch64) fonctionne nativement, plus vite et avec moins d''énergie.' + #13#10#13#10 +
      'Installer quand même la version pour processeurs Intel et AMD ?',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
end;
#endif

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
