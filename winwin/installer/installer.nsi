; The Windows installer. Keep this file UTF-8 with a BOM: without one,
; makensis on Windows reads it in the system code page and the Japanese
; strings below come out garbled.
;
; Built by CI after `cargo build --release`:
;
;   pwsh payload.ps1 -Dir ../target/release -Out payload.nsh
;   makensis /INPUTCHARSET UTF8 /DVERSION=<version> /DPAYLOAD=payload.nsh /DOUTFILE=<setup.exe> installer.nsi
;
; payload.nsh lists winwin.exe and the Windows App Runtime files beside it,
; which the settings window (WinUI 3) needs.
;
; It follows what draftpad's installer does (draftpad/src-tauri/installer.nsi,
; see draftpad/DEVELOPMENT.md): a per-user install without elevation, a
; running copy closed without asking, an upgrade that installs over the
; previous version, no stop on the log page, and a desktop shortcut offered on
; the finish page, unticked when upgrading. Unlike draftpad's, the finish page
; also offers to open the settings window.

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma
; Most of the installer is the Windows App Runtime; a dictionary larger than
; the default 8 MB lets LZMA find more of its repetition.
SetCompressorDictSize 64

!ifndef VERSION
  !error "pass /DVERSION=<version>"
!endif
!ifndef PAYLOAD
  !error "pass /DPAYLOAD=<payload.nsh written by payload.ps1>"
!endif
!include "${PAYLOAD}"
!ifndef OUTFILE
  !define OUTFILE "winwin_${VERSION}_x64-setup.exe"
!endif

!define PRODUCT "winwin"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\winwin"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
; The class of winwin's hidden main window (src/win/app.rs).
!define MAIN_CLASS "winwin.main"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "x64.nsh"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
; Per user, like draftpad's installer: no administrator rights needed. An
; earlier installation's folder wins over the default.
InstallDir "$LOCALAPPDATA\${PRODUCT}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "LegalCopyright" ""

; Set when this run installs over an existing installation. See the finish
; page below.
Var Upgrading
; The finish page's "open the settings" checkbox, and whether it was ticked
; when the page was left.
Var OpenSettingsBox
Var OpenSettings

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
; MUI_FINISHPAGE_NOAUTOCLOSE stays undefined, as in draftpad's installer: the
; installer moves on to the finish page by itself once the files are copied.
!insertmacro MUI_PAGE_INSTFILES

; The "show readme" checkbox of the finish page, repurposed to create the
; desktop shortcut, as Tauri's installer does.
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "デスクトップにショートカットを作成する"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "winwin を起動する"
!define MUI_FINISHPAGE_RUN_FUNCTION RunWinwin
!define MUI_PAGE_CUSTOMFUNCTION_SHOW FinishPageShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE FinishPageLeave
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Japanese"

; Asks a running winwin to quit, the way its own tray menu does, and waits
; for it to go so that its executable can be replaced or removed. No
; confirmation, as in draftpad's installer: the tray icon would not tell the
; user much anyway, and an open settings window (a second winwin.exe, which
; the resident one ends as it quits) is discarded.
!macro CloseRunningWinwin un
  Function ${un}CloseRunningWinwin
    StrCpy $1 0
    loop:
      FindWindow $0 "${MAIN_CLASS}"
      ${If} $0 = 0
        Return
      ${EndIf}
      ${If} $1 = 0
        SendMessage $0 ${WM_CLOSE} 0 0 /TIMEOUT=2000
      ${EndIf}
      Sleep 100
      IntOp $1 $1 + 1
      ; Five seconds, then give up and let the file copy report the lock.
      ${If} $1 < 50
        Goto loop
      ${EndIf}
  FunctionEnd
!macroend
!insertmacro CloseRunningWinwin ""
!insertmacro CloseRunningWinwin "un."

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "winwin は 64 ビット版の Windows でのみ動作します。"
    Abort
  ${EndIf}
  StrCpy $Upgrading 0
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  ${If} $0 != ""
  ${AndIf} ${FileExists} "$0\winwin.exe"
    StrCpy $Upgrading 1
  ${EndIf}
FunctionEnd

; An installation that was kept in place already has whatever shortcuts its
; owner decided to keep, so the desktop shortcut starts unticked there.
;
; The finish page has only the two checkboxes above, so the third, to open the
; settings window, is added here. MUI places the run checkbox at 90u and the
; readme one 20u below it (Contrib/Modern UI 2/Pages/Finish.nsh); this one
; follows at the same spacing. The settings are opened by the winwin that the
; run checkbox starts, so it is greyed out while that one is unticked.
Function FinishPageShow
  ${If} $Upgrading = 1
    ${NSD_Uncheck} $mui.FinishPage.ShowReadme
  ${EndIf}
  ${NSD_CreateCheckbox} 120u 130u 195u 10u "設定画面を開く"
  Pop $OpenSettingsBox
  SetCtlColors $OpenSettingsBox "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  ${NSD_Check} $OpenSettingsBox
  ${NSD_OnClick} $mui.FinishPage.Run FinishPageRunClicked
FunctionEnd

Function FinishPageRunClicked
  Pop $0
  ${NSD_GetState} $mui.FinishPage.Run $0
  ${If} $0 = ${BST_CHECKED}
    EnableWindow $OpenSettingsBox 1
  ${Else}
    EnableWindow $OpenSettingsBox 0
  ${EndIf}
FunctionEnd

; MUI calls this before it acts on its own checkboxes, so RunWinwin sees the
; answer.
Function FinishPageLeave
  ${NSD_GetState} $OpenSettingsBox $OpenSettings
FunctionEnd

Function CreateDesktopShortcut
  CreateShortcut "$DESKTOP\${PRODUCT}.lnk" "$INSTDIR\winwin.exe"
FunctionEnd

Function RunWinwin
  Exec '"$INSTDIR\winwin.exe"'
  ${If} $OpenSettings = ${BST_CHECKED}
    Call OpenSettingsWindow
  ${EndIf}
FunctionEnd

; Starting winwin a second time while it runs makes the running copy open its
; settings (src/win/app.rs), so wait for the copy just started to create its
; main window and start another. The installer is the foreground process and
; passes that right on to any process, so that the settings window, which the
; resident copy starts, comes up in front instead of behind other windows.
Function OpenSettingsWindow
  StrCpy $1 0
  loop:
    FindWindow $0 "${MAIN_CLASS}"
    ${If} $0 = 0
      Sleep 100
      IntOp $1 $1 + 1
      ; Five seconds, then give up: winwin has shown whatever stopped it.
      ${If} $1 < 50
        Goto loop
      ${EndIf}
      Return
    ${EndIf}
  ; ASFW_ANY
  System::Call 'user32::AllowSetForegroundWindow(i -1)'
  Exec '"$INSTDIR\winwin.exe"'
FunctionEnd

Section "Install"
  Call CloseRunningWinwin

  SetOutPath "$INSTDIR"
  !insertmacro INSTALL_PAYLOAD
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\winwin.exe"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\winwin.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "y-saeki"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  SectionGetSize 0 $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" $0
SectionEnd

Section "Uninstall"
  Call un.CloseRunningWinwin

  !insertmacro UNINSTALL_PAYLOAD
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  Delete "$DESKTOP\${PRODUCT}.lnk"

  ; The sign-in entry winwin's settings window writes. Left behind, it would
  ; point at a file that is gone.
  DeleteRegValue HKCU "${RUN_KEY}" "${PRODUCT}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"

  ; The config is the user's work; keep it unless they say otherwise. A
  ; silent uninstall keeps it.
  ${If} ${FileExists} "$APPDATA\${PRODUCT}\*.*"
    MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONQUESTION \
      "ショートカットの設定($APPDATA\${PRODUCT})も削除しますか?" /SD IDNO IDNO keep_config
    RMDir /r "$APPDATA\${PRODUCT}"
    keep_config:
  ${EndIf}
SectionEnd
