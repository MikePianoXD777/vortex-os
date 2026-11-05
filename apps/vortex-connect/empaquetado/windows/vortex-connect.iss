; Instalador de Vortexine Connect para Windows (Inno Setup 6).
;
; Empaca lo que dejó PyInstaller en dist\vortex-connect\ y crea la entrada en el
; menú Inicio. Se instala por usuario, sin pedir permisos de administrador.
;
; Compilar (desde apps\vortex-connect): ISCC.exe /DAppVersion=1.0.0 empaquetado\windows\vortex-connect.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{8D1F4A67-2B9C-4E3D-A5F0-6C7E91B2D358}
AppName=Vortexine Connect
AppVersion={#AppVersion}
AppVerName=Vortexine Connect {#AppVersion}
AppPublisher=Vortexine's
AppPublisherURL=https://github.com/MikePianoXD777/vortex-os
AppSupportURL=https://github.com/MikePianoXD777/vortex-os/issues
DefaultDirName={autopf}\Vortexine Connect
DefaultGroupName=Vortexine Connect
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=VortexineConnect-{#AppVersion}-windows-x64-instalador
SetupIconFile=vortex-connect.ico
UninstallDisplayIcon={app}\vortex-connect.exe
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
; deja sin abrir). Tus ajustes viven en otro lado (%APPDATA%).
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\vortex-connect.exe"

[Files]
Source: "..\..\dist\vortex-connect\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Vortexine Connect"; Filename: "{app}\vortex-connect.exe"
Name: "{group}\{cm:UninstallProgram,Vortexine Connect}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Vortexine Connect"; Filename: "{app}\vortex-connect.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vortex-connect.exe"; Description: "{cm:LaunchProgram,Vortexine Connect}"; Flags: nowait postinstall skipifsilent

[Registry]
; Al desinstalar, el modo viaje no se queda arrancando con Windows.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "VortexineConnect"; ValueType: none; Flags: dontcreatekey uninsdeletevalue
