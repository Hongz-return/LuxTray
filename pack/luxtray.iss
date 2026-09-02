#ifndef MyAppVersion
#define MyAppVersion "0.1.0"
#endif
#define MyAppName "LuxTray"
#define MyAppExeName "LuxTray.exe"
#define MyAppPublisher "LuxTray"

[Setup]
AppId={{B3E91A07-6D24-4C8F-A1B5-9E0C7F2D4A18}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=LuxTray-{#MyAppVersion}-setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
CloseApplications=yes
RestartApplications=no
UsePreviousAppDir=yes
Uninstallable=yes
LanguageDetectionMethod=uilanguage

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
chinesesimplified.DesktopIcon=创建桌面快捷方式
chinesesimplified.AdditionalIcons=附加图标:
chinesesimplified.Autostart=登录时启动 LuxTray
chinesesimplified.StartupGroup=启动:
chinesesimplified.UninstallLuxTray=卸载 LuxTray
chinesesimplified.LaunchNow=立即运行 LuxTray
english.DesktopIcon=Create a desktop shortcut
english.AdditionalIcons=Additional icons:
english.Autostart=Start LuxTray when I sign in
english.StartupGroup=Startup:
english.UninstallLuxTray=Uninstall LuxTray
english.LaunchNow=Launch LuxTray now

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "{cm:Autostart}"; GroupDescription: "{cm:StartupGroup}"

[Files]
Source: "..\dist\LuxTray.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallLuxTray}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "LuxTray"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchNow}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM LuxTray.exe /F"; Flags: runhidden; RunOnceId: "KillLuxTray"

[UninstallDelete]
Type: files; Name: "{userstartup}\LuxTray.lnk"
Type: files; Name: "{userstartup}\GlowTray.lnk"
