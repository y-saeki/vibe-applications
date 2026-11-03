; The Windows installer. Keep this file UTF-8 with a BOM: without one,
; makensis on Windows reads it in the system code page and the Japanese
; strings below come out garbled.
;
; Built by CI after `cargo build --release`:
;
;   makensis /INPUTCHARSET UTF8 /DVERSION=<version> /DOUTFILE=<setup.exe> installer.nsi
;
; It follows winwin's installer (winwin/installer/installer.nsi): a per-user
; install without elevation, a running copy closed without asking, an
; upgrade that installs over the previous version, and the app started from
; the finish page. myaku is a single executable, so there is no payload
; list.

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!ifndef VERSION
  !error "pass /DVERSION=<version>"
!endif
!ifndef OUTFILE
  !define OUTFILE "myaku_${VERSION}_x64-setup.exe"
!endif

!define PRODUCT "myaku"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\myaku"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
; The class of myaku's hidden main window (src/win/app.rs).
!define MAIN_CLASS "myaku.main"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\${PRODUCT}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "LegalCopyright" ""

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "myaku を起動する"
!define MUI_FINISHPAGE_RUN_FUNCTION RunMyaku
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Japanese"

; Asks a running myaku to quit and waits for it to go, so that its
; executable can be replaced or removed.
!macro CloseRunningMyaku un
  Function ${un}CloseRunningMyaku
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
!insertmacro CloseRunningMyaku ""
!insertmacro CloseRunningMyaku "un."

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "myaku は 64 ビット版の Windows でのみ動作します。"
    Abort
  ${EndIf}
FunctionEnd

Function RunMyaku
  Exec '"$INSTDIR\myaku.exe"'
FunctionEnd

Section "Install"
  Call CloseRunningMyaku

  SetOutPath "$INSTDIR"
  File "..\target\release\myaku.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\myaku.exe"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\myaku.exe"
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
  Call un.CloseRunningMyaku

  Delete "$INSTDIR\myaku.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"

  ; The sign-in entry myaku's menu writes. Left behind, it would point at a
  ; file that is gone.
  DeleteRegValue HKCU "${RUN_KEY}" "${PRODUCT}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
