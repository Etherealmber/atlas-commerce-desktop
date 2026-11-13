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

#define AppName "Atlas Commerce"
; Server chosen by the user, or supplied with /SERVERURL=https://...
; The app locks this address: Settings never offers another one.
#define AppExeName "atlas-commerce-desktop.exe"
#define AppIdentity "AtlasCommerceDesktop"

[Setup]
; Never change: this is how Windows tells an update from a new program.
AppId={{E74305A7-4960-45B6-93C9-CEA8D83B1517}
AppName={#AppName}
AppVersion={#Version}
AppVerName={#AppName}
AppPublisher=Etherealmber
AppCopyright=© 2026 loxdpsy/Etherealmber
AppPublisherURL=https://github.com/Etherealmber/atlas-commerce-desktop
AppSupportURL=https://github.com/Etherealmber/atlas-commerce-desktop/issues
AppUpdatesURL=https://github.com/Etherealmber/atlas-commerce-desktop/releases
DefaultDirName={localappdata}\Programs\Atlas Commerce Desktop
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
; One installer for every supported 64-bit Windows PC: x64 machines get the
; Intel/AMD binary, ARM machines the native ARM64 binary (no emulation).
; Space-separated identifier lists, the documented syntax for both
; architecture directives and the form used by production universal
; installers (e.g. ExplorerTabUtility).
ArchitecturesAllowed=x64compatible arm64
ArchitecturesInstallIn64BitMode=x64compatible arm64
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
; Extracted only for the pre-install health check, never installed separately.
Source: "{#BinaryX64}"; DestDir: "{tmp}"; DestName: "atlas-probe-x64.exe"; Flags: dontcopy noencryption
Source: "{#BinaryArm64}"; DestDir: "{tmp}"; DestName: "atlas-probe-arm64.exe"; Flags: dontcopy noencryption solidbreak
; Same destination name for both architectures; the Check functions decide
; which single source is installed on this machine. solidbreak separates
; the two same-named files inside the solid LZMA archive.
Source: "{#BinaryX64}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Check: IsX64; Flags: ignoreversion
Source: "{#BinaryArm64}"; DestDir: "{app}"; DestName: "{#AppExeName}"; Check: IsARM64; Flags: ignoreversion solidbreak
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "atlas-commerce-desktop-installer.txt"; DestDir: "{app}"; Flags: ignoreversion

; Remove the old product-named shortcuts when this same Inno AppId is
; upgraded. The stable AppId and install directory remain unchanged.
[InstallDelete]
Type: files; Name: "{autoprograms}\Atlas Commerce Desktop.lnk"
Type: files; Name: "{autodesktop}\Atlas Commerce Desktop.lnk"; Tasks: desktopicon

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
    case SuppressibleTaskDialogMsgBox('Atlas Commerce ne peut pas encore être supprimé.',
      'Son fichier programme est utilisé ou inaccessible. Quittez Atlas Commerce et réessayez, ou fermez-le ici pour continuer.' + #13#10#13#10 +
      'Fermer Atlas Commerce termine le processus immédiatement.',
      mbError, MB_YESNOCANCEL, ['Réessayer', 'Fermer Atlas Commerce'], 0, IDCANCEL) of
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
  // A fresh install asks again: the installer's remembered address must not
  // survive uninstall.
  RegDeleteValue(HKCU, 'Software\{#AppIdentity}', 'Server');
  if not RegQueryStringValue(HKCU, 'Software\Classes\atlascommerce\shell\open\command', '', Command) then
    Exit;
  Exe := Lowercase(ExpandConstant('{app}\{#AppExeName}'));
  if Pos(Exe, Lowercase(Command)) > 0 then
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\atlascommerce');
end;

{ ── Server chosen at install time ─────────────────────────────────────── }
var
  ServerPage: TInputQueryWizardPage;
  Existing: String;
  HadStoredServer: Boolean;
  ChosenForThisSetup: String;
  PreviousInstall: Boolean;
  VerifiedServer: String;

function ServerUrlFromParam: String;
begin
  Result := Trim(ExpandConstant('{param:SERVERURL|}'));
end;

function ValidServerUrl(const Value: String): Boolean;
var
  Lowered, Authority: String;
  P: Integer;
begin
  Lowered := Lowercase(Trim(Value));
  Result := (Pos('https://', Lowered) = 1)
    or (Pos('http://localhost', Lowered) = 1)
    or (Pos('http://127.0.0.1', Lowered) = 1)
    or (Pos('http://[::1]', Lowered) = 1);
  if not Result then Exit;
  P := Pos('://', Lowered);
  Authority := Copy(Lowered, P + 3, Length(Lowered));
  { An origin only, no credentials, path, query, fragment or control chars.
    The application independently validates and normalizes this again. }
  Result := (Authority <> '') and (Pos('/', Authority) = 0)
    and (Pos('?', Authority) = 0) and (Pos('#', Authority) = 0)
    and (Pos('"', Value) = 0) and (Pos('\', Value) = 0)
    and (Pos('@', Value) = 0) and (Pos(' ', Value) = 0)
    and (Pos(#9, Value) = 0) and (Pos(#10, Value) = 0)
    and (Pos(#13, Value) = 0) and (Length(Value) <= 200);
  if Result and (Pos('http://', Lowered) = 1) then
    Result := (Authority = 'localhost') or (Pos('localhost:', Authority) = 1)
      or (Authority = '127.0.0.1') or (Pos('127.0.0.1:', Authority) = 1)
      or (Authority = '[::1]') or (Pos('[::1]:', Authority) = 1);
end;

procedure InitializeWizard;
begin
  ServerPage := CreateInputQueryPage(wpSelectDir,
    'Serveur Atlas Commerce',
    'Quel serveur cette installation doit-elle utiliser ?',
    'L''adresse est fixée par l''installateur : elle ne pourra plus être changée dans les réglages. Une réinstallation est nécessaire pour en choisir une autre.');
  ServerPage.Add('Adresse du serveur (HTTPS) :', False);
  // Pre-fill order: silent /SERVERURL, then the address this installation
  // already uses (an upgrade keeps it), otherwise leave it blank. The value
  // is written to the registry at the end of setup and removed by uninstall.
  PreviousInstall := RegKeyExists(HKCU,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{E74305A7-4960-45B6-93C9-CEA8D83B1517}_is1');
  HadStoredServer := RegQueryStringValue(HKCU, 'Software\{#AppIdentity}', 'Server', Existing);
  if ServerUrlFromParam <> '' then
    ServerPage.Values[0] := ServerUrlFromParam
  else if HadStoredServer then
    ServerPage.Values[0] := Existing
  else
    ServerPage.Values[0] := '';
end;

function ChosenServer: String;
begin
  Result := Trim(ServerPage.Values[0]);
end;

function ServerNeedsProbe(const Value: String): Boolean;
begin
  { Keep existing installations upgradeable offline, without replacing their
    previously selected server. New or changed choices must be checked. }
  Result := not (PreviousInstall and
    (((Value = '') and (not HadStoredServer)) or
     (HadStoredServer and (CompareText(Value, Trim(Existing)) = 0))));
end;

function CheckChosenServer: Boolean;
var
  Name, Value: String;
  ExitCode: Integer;
begin
  Value := ChosenServer;
  Result := False;
  if not ValidServerUrl(Value) then
  begin
    Result := (Value = '') and PreviousInstall and (not HadStoredServer);
    Exit;
  end;
  if not ServerNeedsProbe(Value) or (Value = VerifiedServer) then
  begin
    Result := True;
    Exit;
  end;
  if IsARM64 then Name := 'atlas-probe-arm64.exe'
  else Name := 'atlas-probe-x64.exe';
  try
    ExtractTemporaryFile(Name);
    Result := Exec(ExpandConstant('{tmp}\' + Name),
      '--installer-probe "' + Value + '"', '', SW_HIDE, ewWaitUntilTerminated, ExitCode)
      and (ExitCode = 0);
  except
    Result := False;
  end;
  if Result then VerifiedServer := Value;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if CurPageID = ServerPage.ID then
  begin
    if not CheckChosenServer then
    begin
      MsgBox('Serveur non vérifié. Vérifiez l’adresse HTTPS, le certificat et la connexion, puis réessayez.', mbError, MB_OK);
      Result := False;
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  { Also enforce the check for silent installations, which skip wizard pages. }
  if not CheckChosenServer then
    Result := 'Serveur non vérifié. Vérifiez l’adresse HTTPS, le certificat et la connexion.';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Data: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  ChosenForThisSetup := ChosenServer;
  // Upgrading an installation that predates this page leaves its existing
  // (possibly self-hosted) address alone: without a stored value or an
  // explicit /SERVERURL we cannot know it, and guessing the public default
  // would silently move a shop to another worker. The app locks whatever it
  // already uses. A fresh install writes the chosen address.
  if (ChosenForThisSetup = '') and (not HadStoredServer) and PreviousInstall then
    Exit;
  { Read once by the app, which then deletes this file and locks the
    address in its own servers.json. A silent install writes it too. }
  Data := '{"url":"' + ChosenForThisSetup + '","label":"Atlas Commerce"}';
  if not SaveStringToFile(ExpandConstant('{app}\server.json'), Data, False) then
    MsgBox('Le serveur n''a pas pu être enregistré. L''application demandera l''adresse au premier lancement.', mbInformation, MB_OK);
  RegWriteStringValue(HKCU, 'Software\{#AppIdentity}', 'Server', ChosenForThisSetup);
end;
