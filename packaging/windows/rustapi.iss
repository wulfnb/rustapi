; Per-user installer for RustAPI. Running a newer installer upgrades in place:
; the fixed AppId reuses the previous directory and uninstall entry.
; CD passes /DAppVersion=<Cargo version>[+<commit>] so each build is distinguishable.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
[Setup]
AppId=Wulfnb.RustAPI
AppName=RustAPI
AppVersion={#AppVersion}
AppVerName=RustAPI {#AppVersion}
DefaultDirName={localappdata}\Programs\RustAPI
DefaultGroupName=RustAPI
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=RustAPI-windows-x86_64-Setup
SetupIconFile=..\..\assets\icons\RustAPIIcon.ico
UninstallDisplayIcon={app}\rustapi.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; An open RustAPI locks its executable; close it before replacing files.
CloseApplications=yes
RestartApplications=no

; Remove the previous version's binaries first so nothing dropped in this release lingers.
[InstallDelete]
Type: files; Name: "{app}\*.dll"
Type: files; Name: "{app}\rustapi.exe"

[Files]
Source: "..\..\dist\RustAPI-windows-x86_64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\RustAPI"; Filename: "{app}\rustapi.exe"
Name: "{group}\Uninstall RustAPI"; Filename: "{uninstallexe}"
Name: "{autodesktop}\RustAPI"; Filename: "{app}\rustapi.exe"; Tasks: desktopicon

[Tasks]
Name: desktopicon; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Run]
Filename: "{app}\rustapi.exe"; Description: "Open RustAPI"; Flags: nowait postinstall skipifsilent

