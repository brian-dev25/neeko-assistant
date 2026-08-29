$UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"

$puuid = "zbifMTI2wdGO7Y9-6ZMqowoFMNsTJy7Gk28aq0vTAMtz0jRwWnBQrXTCWsIII0RPXQ485_uaTWPnSA"

# Obtener games con puuid - solo 1 game para ver estructura completa
$url = "https://lol-api-summoner.op.gg/api/v3/las/summoners/$puuid/games?limit=1&game_type=total&hl=es_MX"
$r = Invoke-WebRequest -Uri $url -Headers @{ "User-Agent" = $UA } -UseBasicParsing

# Parsear el JSON raw
$json = $r.Content | ConvertFrom-Json
$game = $json.data[0]

Write-Host "=== GAME INFO ===" -ForegroundColor Cyan
Write-Host "game_type: $($game.game_type)"
Write-Host "queue_id: $($game.queue_id)"
Write-Host "game_length_second: $($game.game_length_second)"
Write-Host "created_at: $($game.created_at)"
Write-Host "game_map: $($game.game_map)"

# Buscar al jugador en participants
foreach ($p in $game.participants) {
    if ($p.summoner.puuid -eq $puuid -or $p.participant_id -ne $null) {
        Write-Host "`n=== PARTICIPANT RAW ===" -ForegroundColor Yellow
        # Obtener el JSON raw del participant
        $pJson = $p | ConvertTo-Json -Depth 10
        Write-Host $pJson
        break
    }
}
