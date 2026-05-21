; Inno Setup script — compile on Windows with: iscc installer\roust.iss
; Run installer\stage.ps1 first to populate installer\staging\

#define MyAppName "roust"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "roust"
#define MyAppURL "https://github.com/ArminDashti/roust"
#define MyAppExeName "roust.exe"
#define MySetupExeName "roust-setup.exe"

[Setup]
AppId={{A7B3C9E1-4F2D-4A8B-9C0E-123456789ABC}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\WinDivert-2.2.2-A\LICENSE
OutputDir=output
OutputBaseFilename=roust-setup-{#MyAppVersion}-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "staging\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\{#MySetupExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\private_ips.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\WinDivert-2.2.2-A\*"; DestDir: "{app}\WinDivert-2.2.2-A"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName} (command help)"; Filename: "{cmd}"; Parameters: "/k cd /d ""{app}"" && {#MyAppExeName} --help"
Name: "{group}\{#MyAppName} folder"; Filename: "{app}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{cmd}"; Parameters: "/k cd /d ""{app}"" && {#MyAppExeName} --help"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MySetupExeName}"; Parameters: "--dir ""{app}"" --skip-windivert"; StatusMsg: "Downloading IP lists and updating PATH..."; Flags: waituntilterminated

[UninstallRun]
Filename: "{app}\{#MySetupExeName}"; Parameters: "--dir ""{app}"" --uninstall-path"; RunOnceId: "RemoveRoustPath"; Flags: waituntilterminated skipifdoesntexist
