Unicode true
!include "MUI2.nsh"
Name "Skarva"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Skarva"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "InstallLocation"
RequestExecutionLevel user
ManifestDPIAware true
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey "ProductName" "Skarva"
VIAddVersionKey "FileDescription" "Skarva installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright David Lockley"
!define MUI_ABORTWARNING
!define MUI_ICON "app-icon.ico"
!define MUI_UNICON "app-icon.ico"
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Skarva"
    SetShellVarContext current
    SetOutPath "$INSTDIR"
    File /r "${PACKAGE_DIR}\*"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateDirectory "$SMPROGRAMS\Skarva"
    CreateShortcut "$SMPROGRAMS\Skarva\Skarva.lnk" "$INSTDIR\skarva.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "DisplayName" "Skarva"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "DisplayVersion" "${APP_VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "DisplayIcon" "$INSTDIR\skarva.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "Publisher" "David Lockley"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva" "NoRepair" 1
SectionEnd

Section "Uninstall"
    SetShellVarContext current
    Delete "$SMPROGRAMS\Skarva\Skarva.lnk"
    RMDir "$SMPROGRAMS\Skarva"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Skarva"
    Delete "$INSTDIR\skarva.exe"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\NOTICE"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\THIRD_PARTY_NOTICES.md"
    Delete "$INSTDIR\licenses\Apache-2.0.txt"
    Delete "$INSTDIR\licenses\THIRD_PARTY.txt"
    RMDir "$INSTDIR\licenses"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
SectionEnd
