; GeoD Global installer. No system-wide changes or application-data removal.
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
VIAddVersionKey "FileDescription" "GeoD Global Windows installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "GeoD Global contributors"
!define MUI_ABORTWARNING
!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "$(RemoveText)"
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"
LangString WelcomeText ${LANG_ENGLISH} "Install GeoD Global ${APP_VERSION} for the current user.$\r$\n$\r$\nMicrosoft Edge WebView2 is required. Quit GeoD Global from the system tray before upgrading.$\r$\n$\r$\nUninstall retains your downloaded files, task history and preferences. This release candidate is unsigned."
LangString WelcomeText ${LANG_SIMPCHINESE} "安装 GeoD Global ${APP_VERSION}，仅为当前用户安装。$\r$\n$\r$\n需要 Microsoft Edge WebView2。升级前请从系统托盘退出 GeoD Global。$\r$\n$\r$\n卸载将保留下载文件、任务历史和偏好。本发布候选尚未签名。"
LangString RemoveText ${LANG_ENGLISH} "Remove GeoD Global application files? Downloaded files, task history, preferences and additional files in the installation directory will be retained."
LangString RemoveText ${LANG_SIMPCHINESE} "卸载 GeoD Global 应用程序？下载文件、任务历史、偏好和安装目录中的额外文件将保留。"

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
