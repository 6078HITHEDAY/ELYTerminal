# ELYTerminal: wrap the prompt to report folder and title.
$global:ElyPrompt = $function:prompt
function global:prompt {
    $folder = $executionContext.SessionState.Path.CurrentLocation
    if ($folder.Provider.Name -eq 'FileSystem') {
        $path = $folder.ProviderPath.Replace('\', '/').Replace('%', '%25')
        if (-not $path.StartsWith('/')) { $path = "/$path" }
        $leaf = Split-Path -Leaf $folder.ProviderPath
        $esc, $bel = [char]27, [char]7
        [Console]::Write("$esc]7;file://$path$bel$esc]2;$leaf$bel")
    }
    & $global:ElyPrompt
}
