$link = "https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/EndercordInstallerCli.exe"

$outfile = "$env:TEMP\EndercordInstallerCli.exe"

Write-Output "Downloading installer to $outfile"

Invoke-WebRequest -Uri "$link" -OutFile "$outfile"

Write-Output ""

Start-Process -Wait -NoNewWindow -FilePath "$outfile"

# Cleanup
Remove-Item -Force "$outfile"
