; GeoD Global evaluation installer. No system-wide changes or application-data removal.
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!define APP_ID "xyz.laogao.geod.global"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
Name "GeoD Global"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\GeoD Global"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${APP_NUMERIC_VERSION}"
VIAddVersionKey "ProductName" "GeoD Global"
VIAddVersionKey "FileDescription" "GeoD Global unsigned local evaluation installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "GeoD Global contributors"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "This unsigned evaluation package installs GeoD Global for the current user.$\r$\n$\r$\nMicrosoft Edge WebView2 must already be installed. This installer does not download or install prerequisites.$\r$\n$\r$\nUninstall removes packaged application files only. Your downloaded rasters, recipes, task history and preferences are retained."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Remove GeoD Global application files? Local rasters, recipes, task history, preferences and any additional files in the installation directory will be retained."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "GeoD Global requires Windows x64."
    Abort
  ${EndIf}
  ; Read-only prerequisite detection; no WebView2 installer or registry mutation.
  ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 == ""
    SetRegView 32
    ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
    SetRegView 64
    ${If} $0 == ""
      ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
    ${EndIf}
  ${EndIf}
  ${If} $0 == ""
    MessageBox MB_ICONSTOP "Microsoft Edge WebView2 Runtime is required. Install it from Microsoft's official website, then run this installer again. See README.md in the portable package."
    Abort
  ${EndIf}
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Section "GeoD Global"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD}\*"
  WriteUninstaller "$INSTDIR\Uninstall GeoD Global.exe"
  CreateShortcut "$SMPROGRAMS\GeoD Global.lnk" "$INSTDIR\geod-global-desktop.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "GeoD Global"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "GeoD Global"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall GeoD Global.exe$\"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  ; Generated explicit packaged filenames only. Never recursively delete directories.
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\Uninstall GeoD Global.exe"
  RMDir "$INSTDIR"
  ; A newer installation may have moved to another directory. Its shared entries belong to it.
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  ${If} $0 == "$INSTDIR"
    Delete "$SMPROGRAMS\GeoD Global.lnk"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ${EndIf}
  ; Deliberately do not touch $LOCALAPPDATA\${APP_ID} or $APPDATA\${APP_ID}.
SectionEnd
