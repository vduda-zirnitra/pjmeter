#define MyAppName "PJMeter"
#define MyAppVersion GetEnv("CARGO_VERSION")
#define MyAppPublisher "Victor Duda"
#define MyAppURL "https://github.com/vduda-zirnitra/pjmeter"
#define MyAppExeName "pjmeter.exe"
#define SourceDir GetEnv("SOURCE_DIR")
#define JMeterVersion GetEnv("JMETER_VERSION")

[Setup]
; NOTE: The value of AppId uniquely identifies this application. Do not use the same AppId value in installers for other applications.
AppId={{8C9F6E4A-5B3D-4F2A-9E1C-7D8A6B5C4E3F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile={#SourceDir}\LICENSE
OutputDir={#SourceDir}\build\windows
OutputBaseFilename=pjmeter_{#MyAppVersion}_windows_x86_64_installer
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "Add to PATH environment variable"; GroupDescription: "System Integration:"; Flags: checkedonce

[Files]
Source: "{#SourceDir}\target\x86_64-pc-windows-gnu\release\{#MyAppExeName}"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#SourceDir}\.jmeter-app\apache-jmeter-{#JMeterVersion}\*"; DestDir: "{app}\jmeter"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Dirs]
Name: "{app}\jmeter_plugins"
Name: "{commonappdata}\pjmeter"; Permissions: users-modify

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{cmd}"; Parameters: "/k ""{app}\bin\{#MyAppExeName}"" --help"; WorkingDir: "{userdocs}"; Comment: "Open {#MyAppName} command line"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{cmd}"; Parameters: "/k ""{app}\bin\{#MyAppExeName}"" --help"; WorkingDir: "{userdocs}"; Tasks: desktopicon; Comment: "Open {#MyAppName} command line"

[Code]
const EnvironmentKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

procedure EnvAddPath(Path: string);
var
    Paths: string;
begin
    { Retrieve current path (use empty string if entry not exists) }
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Paths := '';

    { Skip if string already found in path }
    if Pos(';' + Uppercase(Path) + ';', ';' + Uppercase(Paths) + ';') > 0 then exit;

    { App string to the end of the path variable }
    Paths := Paths + ';' + Path;

    { Overwrite (or create if missing) path environment variable }
    if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Log(Format('The [%s] added to PATH: [%s]', [Path, Paths]))
    else Log(Format('Error while adding the [%s] to PATH: [%s]', [Path, Paths]));
end;

procedure EnvRemovePath(Path: string);
var
    Paths: string;
    P: Integer;
begin
    { Skip if registry entry not exists }
    if not RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths) then
        exit;

    { Skip if string not found in path }
    P := Pos(';' + Uppercase(Path) + ';', ';' + Uppercase(Paths) + ';');
    if P = 0 then exit;

    { Update path variable }
    Delete(Paths, P - 1, Length(Path) + 1);

    { Overwrite path environment variable }
    if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', Paths)
    then Log(Format('The [%s] removed from PATH: [%s]', [Path, Paths]))
    else Log(Format('Error while removing the [%s] from PATH: [%s]', [Path, Paths]));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
    if CurStep = ssPostInstall then
    begin
        if IsTaskSelected('addtopath') then
            EnvAddPath(ExpandConstant('{app}\bin'));
    end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
    if CurUninstallStep = usPostUninstall then
        EnvRemovePath(ExpandConstant('{app}\bin'));
end;

[Run]
Filename: "{cmd}"; Parameters: "/C echo Installation complete! Close this window to continue."; Flags: postinstall nowait skipifsilent runhidden
