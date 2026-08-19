#ifndef AppVersion
#define AppVersion "0.1.4"
#endif

#ifndef AppArch
#define AppArch "x64"
#endif

#ifndef SourceDir
#define SourceDir "."
#endif

#ifndef OutputDir
#define OutputDir "."
#endif

#ifndef IconFile
#define IconFile ".\icon.ico"
#endif

#ifndef LicenseFile
#define LicenseFile ".\LICENSE"
#endif

[Setup]
AppId={{B7B7C5DF-EE40-4D67-8D99-9DD904A59D2C}
AppName=gds3d
AppVersion={#AppVersion}
AppPublisher=gds3d
DefaultDirName={autopf}\gds3d
DefaultGroupName=gds3d
DisableProgramGroupPage=yes
LicenseFile={#LicenseFile}
OutputDir={#OutputDir}
OutputBaseFilename=gds3d-{#AppVersion}-windows-{#AppArch}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\gds3d.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\gds3d"; Filename: "{app}\gds3d.exe"; IconFilename: "{app}\icon.ico"
Name: "{autodesktop}\gds3d"; Filename: "{app}\gds3d.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\gds3d.exe"; Description: "{cm:LaunchProgram,gds3d}"; Flags: nowait postinstall skipifsilent
