; Per-user installer for RustAPI.
[Setup]
AppId=Wulfnb.RustAPI
AppName=RustAPI
AppVersion=0.1.0
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

