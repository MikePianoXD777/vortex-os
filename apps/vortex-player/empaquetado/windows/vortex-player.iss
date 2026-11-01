; Instalador de Vortex Player para Windows (Inno Setup 6).
;
; Empaca lo que dejó PyInstaller en dist\vortex-player\ (con su carpeta bin\) y crea la entrada en el
; menú Inicio. Se instala por usuario, sin pedir permisos de administrador.
;
; Compilar (desde apps\vortex-player): ISCC.exe /DAppVersion=1.3.0 empaquetado\windows\vortex-player.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{3B8E5D21-7C4A-4F6B-9A1E-2D7C5F8B9E40}
AppName=Vortex Player
AppVersion={#AppVersion}
AppVerName=Vortex Player {#AppVersion}
AppPublisher=Vortexine's
AppPublisherURL=https://github.com/MikePianoXD777/vortex-os
AppSupportURL=https://github.com/MikePianoXD777/vortex-os/issues
DefaultDirName={autopf}\Vortex Player
DefaultGroupName=Vortex Player
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=VortexPlayer-{#AppVersion}-windows-x64-instalador
SetupIconFile=vortex-player.ico
UninstallDisplayIcon={app}\vortex-player.exe
LicenseFile=..\..\..\..\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Si está abierto, se cierra para poder reemplazarlo
CloseApplications=yes

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; Al actualizar se borra la versión anterior antes de copiar (mezclar DLLs de dos versiones de Qt la
; deja sin abrir). Tu música, tu biblioteca y tus ajustes viven en otro lado (Música y %APPDATA%).
Type: filesandordirs; Name: "{app}\_internal"
Type: filesandordirs; Name: "{app}\bin"
Type: files; Name: "{app}\vortex-player.exe"

[Files]
Source: "..\..\dist\vortex-player\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Vortex Player"; Filename: "{app}\vortex-player.exe"
Name: "{group}\{cm:UninstallProgram,Vortex Player}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Vortex Player"; Filename: "{app}\vortex-player.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vortex-player.exe"; Description: "{cm:LaunchProgram,Vortex Player}"; Flags: nowait postinstall skipifsilent
