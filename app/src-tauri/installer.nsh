; Custom NSIS hook: make the .md / .markdown / .mdown / .mkd / .txt file
; association show a dedicated document icon.
;
; Two ProgIDs get the correct DefaultIcon:
;   - "Markdown Document" (Tauri's default FILECLASS from tauri.conf.json
;     fileAssociations[0].name)
;   - "ZeroMD.md" (leftover from earlier 1.1.6 rebuilds; UserChoice on some
;     machines still points at it and Windows protects UserChoice so we
;     can't delete it without taking ownership — patch the ProgID instead)

!macro NSIS_HOOK_POSTINSTALL
  ; Primary: override Tauri-registered DefaultIcon for "Markdown Document"
  WriteRegStr SHCTX "Software\Classes\Markdown Document\DefaultIcon" "" '"$INSTDIR\icons\file_icon.ico",0'

  ; Fallback: ensure the ZeroMD.md / ZeroMD.txt ProgIDs (if UserChoice or
  ; older installs point to them) also have the right icon + open command.
  WriteRegStr SHCTX "Software\Classes\ZeroMD.md" "" "Markdown Document"
  WriteRegStr SHCTX "Software\Classes\ZeroMD.md\DefaultIcon" "" '"$INSTDIR\icons\file_icon.ico",0'
  WriteRegStr SHCTX "Software\Classes\ZeroMD.md\shell\open\command" "" '"$INSTDIR\ZeroMD.exe" "%1"'
  WriteRegStr SHCTX "Software\Classes\ZeroMD.txt" "" "Plain Text"
  WriteRegStr SHCTX "Software\Classes\ZeroMD.txt\DefaultIcon" "" '"$INSTDIR\icons\file_icon.ico",0'
  WriteRegStr SHCTX "Software\Classes\ZeroMD.txt\shell\open\command" "" '"$INSTDIR\ZeroMD.exe" "%1"'

  ; #453 — Do NOT hijack .txt's default class ("txtfile"), which breaks Windows
  ; Explorer's right-click "New -> Text Document (.txt)". Self-heal older
  ; installs that wrote "Plain Text" or "ZeroMD.txt" as the default for .txt,
  ; and register ZeroMD.txt under OpenWithProgids instead.
  ReadRegStr $0 HKCU "Software\Classes\.txt" ""
  ${If} $0 == "Plain Text"
  ${OrIf} $0 == "ZeroMD.txt"
    WriteRegStr HKCU "Software\Classes\.txt" "" "txtfile"
  ${EndIf}
  ReadRegStr $0 HKLM "Software\Classes\.txt" ""
  ${If} $0 == "Plain Text"
  ${OrIf} $0 == "ZeroMD.txt"
    WriteRegStr HKLM "Software\Classes\.txt" "" "txtfile"
  ${EndIf}
  WriteRegStr SHCTX "Software\Classes\.txt\OpenWithProgids" "ZeroMD.txt" ""

  ; #429 — Windows Explorer right-click "New -> Markdown Document (.md)"
  WriteRegStr SHCTX "Software\Classes\.md\ShellNew" "NullFile" ""
  WriteRegStr SHCTX "Software\Classes\.md\Markdown Document\ShellNew" "NullFile" ""
  WriteRegStr SHCTX "Software\Classes\.md\ZeroMD.md\ShellNew" "NullFile" ""

  ; Force Explorer to refresh icon cache (SHCNE_ASSOCCHANGED).
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Classes\.md\ShellNew"
  DeleteRegKey SHCTX "Software\Classes\.md\Markdown Document\ShellNew"
  DeleteRegKey SHCTX "Software\Classes\.md\ZeroMD.md\ShellNew"
  DeleteRegValue SHCTX "Software\Classes\.txt\OpenWithProgids" "ZeroMD.txt"
  ReadRegStr $0 HKCU "Software\Classes\.txt" ""
  ${If} $0 == "Plain Text"
  ${OrIf} $0 == "ZeroMD.txt"
    WriteRegStr HKCU "Software\Classes\.txt" "" "txtfile"
  ${EndIf}
  DeleteRegKey SHCTX "Software\Classes\ZeroMD.md"
  DeleteRegKey SHCTX "Software\Classes\ZeroMD.txt"
!macroend
