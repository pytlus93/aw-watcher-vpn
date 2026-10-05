# Passive Windows collector. Does not query remote hosts or change networking.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$interfaces = @(Get-NetIPInterface -PolicyStore ActiveStore -ErrorAction Stop)
$routes = @(Get-NetRoute -PolicyStore ActiveStore -ErrorAction Stop | Where-Object { $_.State -eq 'Alive' } | ForEach-Object {
    @{ interface_index = [uint32]$_.InterfaceIndex; prefix = [string]$_.DestinationPrefix; next_hop = [string]$_.NextHop; metric = [uint32]$_.RouteMetric }
})
$adapters = @([System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() | ForEach-Object {
    $nic = $_
    $props = $nic.GetIPProperties()
    $v4 = if ($nic.Supports([System.Net.NetworkInformation.NetworkInterfaceComponent]::IPv4)) { $props.GetIPv4Properties() } else { $null }
    $v6 = if ($nic.Supports([System.Net.NetworkInformation.NetworkInterfaceComponent]::IPv6)) { $props.GetIPv6Properties() } else { $null }
    $index = if ($v4) { $v4.Index } elseif ($v6) { $v6.Index } else { 0 }
    $metric4 = @($interfaces | Where-Object { $_.InterfaceIndex -eq $index -and $_.AddressFamily -eq 'IPv4' })
    $metric6 = @($interfaces | Where-Object { $_.InterfaceIndex -eq $index -and $_.AddressFamily -eq 'IPv6' })
    @{ network_type = [string]$nic.NetworkInterfaceType; id = [string]$nic.Id; index = [uint32]$index; name = [string]$nic.Name; description = [string]$nic.Description
       up = ($nic.OperationalStatus -eq [System.Net.NetworkInformation.OperationalStatus]::Up)
       addresses = @($props.UnicastAddresses | ForEach-Object { $_.Address.ToString() })
       dns_suffix = [string]$props.DnsSuffix
       dns_servers = @($props.DnsAddresses | ForEach-Object { $_.ToString() })
       metric_v4 = $(if ($metric4.Count) { [uint32]$metric4[0].InterfaceMetric } else { [uint32]::MaxValue })
       metric_v6 = $(if ($metric6.Count) { [uint32]$metric6[0].InterfaceMetric } else { [uint32]::MaxValue }) }
})
@{ schema_version = 1; observed_at = [DateTime]::UtcNow.ToString('o'); adapters = $adapters; routes = $routes } | ConvertTo-Json -Depth 8 -Compress
