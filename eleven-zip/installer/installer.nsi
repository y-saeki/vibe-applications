; The Windows installer. Keep this file UTF-8 with a BOM: without one,
; makensis on Windows reads it in the system code page and the Japanese
; strings below come out garbled.
;
; Built by CI after `cargo build --release`:
;
;   pwsh payload.ps1 -Dir ../target/release -Out payload.nsh
;   makensis /INPUTCHARSET UTF8 /DVERSION=<version> [/DBUILD=<commit>] /DPAYLOAD=payload.nsh /DOUTFILE=<setup.exe> installer.nsi
;
; payload.nsh lists eleven-zip.exe and the Windows App Runtime files beside
; it, which the settings window (WinUI 3) needs.
;
; It follows winwin's installer (winwin/installer/installer.nsi): a per-user
; install without elevation, a running copy closed without asking, an
; upgrade that installs over the previous version, and no stop on the log
; page. On top of that it registers eleven-zip with Explorer, and the
; uninstaller removes that again along with the 7-Zip eleven-zip downloaded.

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
; A pull request's build passes the commit it came from as /DBUILD, which
; follows the version as semver build metadata (0.1.2+047ba8f).
; VIProductVersion takes digits only, so it keeps reading VERSION.
!ifdef BUILD
  !define DISPLAY_VERSION "${VERSION}+${BUILD}"
!else
  !define DISPLAY_VERSION "${VERSION}"
!endif
!ifndef PAYLOAD
  !error "pass /DPAYLOAD=<payload.nsh written by payload.ps1>"
!endif
!include "${PAYLOAD}"
!ifndef OUTFILE
  !define OUTFILE "eleven-zip_${DISPLAY_VERSION}_x64-setup.exe"
!endif

!define PRODUCT "eleven-zip"
!define EXE "eleven-zip.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\eleven-zip"
; The settings window (src/win/settings_ui.rs): WinUI 3's class for a
; desktop window, and its title (src/title.rs), alone as up to 0.1.1 or
; followed by " - v<version>".
!define WINDOW_CLASS "WinUIDesktopWin32WindowClass"
!define WINDOW_TITLE "eleven-zip の設定"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
; Per user: no administrator rights needed. An earlier installation's
; folder wins over the default.
InstallDir "$LOCALAPPDATA\${PRODUCT}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "ProductVersion" "${DISPLAY_VERSION}"
VIAddVersionKey "FileVersion" "${DISPLAY_VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "LegalCopyright" ""

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
; MUI_FINISHPAGE_NOAUTOCLOSE stays undefined: the installer moves on to the
; finish page by itself once the files are copied.
!insertmacro MUI_PAGE_INSTFILES

; Opening the settings window also downloads 7-Zip, so that the first
; extraction does not wait for it.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "eleven-zip の設定を開く"
!define MUI_FINISHPAGE_RUN_FUNCTION OpenSettings
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Japanese"

; Leaves an open settings window in $0, or 0. Its title carries the version,
; which may be the previous one, so this walks the windows of its class and
; matches the start of the title as src/title.rs does.
!macro FindSettings un
  Function ${un}FindSettings
    StrCpy $0 0
    StrLen $4 "${WINDOW_TITLE} - v"
    loop:
      FindWindow $0 "${WINDOW_CLASS}" "" 0 $0
      ${If} $0 = 0
        Return
      ${EndIf}
      System::Call 'user32::GetWindowTextW(p r0, t .r2, i ${NSIS_MAX_STRLEN})'
      ${If} $2 == "${WINDOW_TITLE}"
        Return
      ${EndIf}
      StrCpy $3 $2 $4
      ${If} $3 == "${WINDOW_TITLE} - v"
        Return
      ${EndIf}
      Goto loop
  FunctionEnd
!macroend
!insertmacro FindSettings ""
!insertmacro FindSettings "un."

; Closes an open settings window, the only eleven-zip that stays running, so
; that its files can be replaced or removed. An extraction in progress is a
; 7-Zip window and is left alone.
!macro CloseSettings un
  Function ${un}CloseSettings
    StrCpy $1 0
    loop:
      Call ${un}FindSettings
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
!insertmacro CloseSettings ""
!insertmacro CloseSettings "un."

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "eleven-zip は 64 ビット版の Windows でのみ動作します。"
    Abort
  ${EndIf}
FunctionEnd

Function OpenSettings
  Exec '"$INSTDIR\${EXE}"'
FunctionEnd

Section "Install"
  Call CloseSettings

  SetOutPath "$INSTDIR"
  !insertmacro INSTALL_PAYLOAD
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${EXE}"

  ; The context menu, 「プログラムから開く」 and the default-apps page, all
  ; under HKEY_CURRENT_USER (src/shell.rs).
  ExecWait '"$INSTDIR\${EXE}" --register'

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${DISPLAY_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
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
  Call un.CloseSettings

  ; While the executable that knows what it registered is still there.
  ExecWait '"$INSTDIR\${EXE}" --unregister'

  !insertmacro UNINSTALL_PAYLOAD
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"

  ; The 7-Zip eleven-zip downloaded for itself (src/store.rs). Nothing else
  ; uses it.
  RMDir /r "$LOCALAPPDATA\${PRODUCT}\7-Zip"
  RMDir "$LOCALAPPDATA\${PRODUCT}"

  DeleteRegKey HKCU "${UNINSTALL_KEY}"

  ; The settings are the user's; keep them unless they say otherwise. A
  ; silent uninstall keeps them.
  ${If} ${FileExists} "$APPDATA\${PRODUCT}\*.*"
    MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONQUESTION \
      "eleven-zip の設定($APPDATA\${PRODUCT})も削除しますか?" /SD IDNO IDNO keep_config
    RMDir /r "$APPDATA\${PRODUCT}"
    keep_config:
  ${EndIf}
SectionEnd
