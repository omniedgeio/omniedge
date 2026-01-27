; Optional: Add a section for the background service
; This will show up in the components page when oneClick is false

Section "Background Service" SEC_SERVICE
  SectionIn RO ; Optional: Remove this if you want it to be truly optional
  DetailPrint "Registering OmniEdge Helper Service..."
  ; Note: binomial quoting is tricky in NSIS/sc
  ExecWait 'sc create OmniEdgeHelper binPath= "\"$INSTDIR\omni-helper-x86_64-pc-windows-msvc.exe\"" start= auto DisplayName= "OmniEdge Helper Service"'
  ExecWait 'sc description OmniEdgeHelper "Facilitates secure P2P networking for OmniEdge."'
  ExecWait 'sc start OmniEdgeHelper'
SectionEnd

!macro customUnInstall
  DetailPrint "Stopping OmniEdge Helper Service..."
  ExecWait 'sc stop OmniEdgeHelper'
  DetailPrint "Removing OmniEdge Helper Service..."
  ExecWait 'sc delete OmniEdgeHelper'
!macroend
