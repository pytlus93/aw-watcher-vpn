# Embedded desktop UI. The bootstrap supplies decoded config/executable paths.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()
$keyBytes = [Text.Encoding]::UTF8.GetBytes($configPath.ToLowerInvariant())
$sha = [Security.Cryptography.SHA256]::Create()
$key = [BitConverter]::ToString($sha.ComputeHash($keyBytes)).Replace('-', '')
$sha.Dispose()
$created = $false
$mutex = New-Object Threading.Mutex($true, "Local\aw-watcher-vpn-manager-$key", [ref]$created)
if (-not $created) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class VpnManagerWindow {
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string title);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr w, int mode);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
}
'@
    $window = [VpnManagerWindow]::FindWindow($null, "ActivityWatch - VPN connections ($configPath)")
    if ($window -ne [IntPtr]::Zero -and -not $notificationMessage) {
        [void][VpnManagerWindow]::ShowWindow($window, 5)
        [void][VpnManagerWindow]::SetForegroundWindow($window)
    }
    $mutex.Dispose()
    exit
}
$form = New-Object Windows.Forms.Form
$form.Text = "ActivityWatch - VPN connections ($configPath)"
$form.Size = New-Object Drawing.Size(950, 510)
$form.MinimumSize = New-Object Drawing.Size(680, 440)
$form.StartPosition = 'CenterScreen'
$form.Padding = New-Object Windows.Forms.Padding(12)
$layout = New-Object Windows.Forms.TableLayoutPanel
$layout.Dock = 'Fill'
$layout.ColumnCount = 1
$layout.RowCount = 6
foreach ($height in @(38, 0, 24, 32, 8, 80)) {
    $style = New-Object Windows.Forms.RowStyle
    if ($height -eq 0) { $style.SizeType = 'Percent'; $style.Height = 100 }
    else { $style.SizeType = 'Absolute'; $style.Height = $height }
    [void]$layout.RowStyles.Add($style)
}
$form.Controls.Add($layout)
$intro = New-Object Windows.Forms.Label
$intro.Text = 'All remembered connections and profiles. Select one to name, rename, delete, ignore or restore it.'
$intro.Dock = 'Fill'
$layout.Controls.Add($intro, 0, 0)
$list = New-Object Windows.Forms.ListView
$list.View = 'Details'
$list.FullRowSelect = $true
$list.MultiSelect = $false
$list.Dock = 'Fill'
[void]$list.Columns.Add('Connection', 180)
[void]$list.Columns.Add('Status', 100)
[void]$list.Columns.Add('Suggested / previous customer', 310)
[void]$list.Columns.Add('Connection / profile ID', 300)
$layout.Controls.Add($list, 0, 1)
$label = New-Object Windows.Forms.Label
$label.Text = 'Customer name (choose an existing customer to add this connection)'
$label.Dock = 'Fill'
$layout.Controls.Add($label, 0, 2)
$name = New-Object Windows.Forms.ComboBox
$name.Dock = 'Fill'
$name.DropDownStyle = 'DropDown'
$layout.Controls.Add($name, 0, 3)
$buttons = New-Object Windows.Forms.FlowLayoutPanel
$buttons.Dock = 'Fill'
$save = New-Object Windows.Forms.Button
$save.Text = 'Save / add to customer'
$save.AutoSize = $true
$refresh = New-Object Windows.Forms.Button
$refresh.Text = 'Refresh'
$refresh.AutoSize = $true
$ignore = New-Object Windows.Forms.Button
$ignore.Text = 'Ignore selected'
$ignore.AutoSize = $true
$ignoredSettings = New-Object Windows.Forms.Button
$ignoredSettings.Text = 'Ignored VPNs...'
$ignoredSettings.AutoSize = $true
$status = New-Object Windows.Forms.Label
$status.AutoSize = $true
$status.Padding = New-Object Windows.Forms.Padding(8)
$payloadSettings = New-Object Windows.Forms.Button
$payloadSettings.Text = 'Payload...'
$payloadSettings.AutoSize = $true
$delete = New-Object Windows.Forms.Button
$delete.Text = 'Delete connection'
$delete.AutoSize = $true
$delete.Enabled = $false
$buttons.Controls.AddRange(@($save, $delete, $ignore, $ignoredSettings, $payloadSettings, $refresh, $status))
$layout.Controls.Add($buttons, 0, 5)

function Refresh-Connections {
    try {
        $script:refreshing = $true
        $selected = if ($list.SelectedItems.Count) { $list.SelectedItems[0].Tag.id } else { $null }
        $output = & $watcherExe connections $configPath 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        $data = ($output -join [Environment]::NewLine) | ConvertFrom-Json
        $script:ignoredConnections = @($data.ignored_connections)
        $ids = @($data.connections | Where-Object { -not $_.assigned -and -not $_.ignored } | ForEach-Object id)
        $script:notifyEnabled = $data.notify
        if ($notificationMessage -and $data.notify -and $script:loaded -and @($ids | Where-Object { $script:previousIds -notcontains $_ }).Count -gt 0) {
            $tray.ShowBalloonTip(10000, 'ActivityWatch VPN', 'New unnamed VPN connections detected. Click to name them.', [Windows.Forms.ToolTipIcon]::Info)
        }
        $script:previousIds = $ids
        $script:loaded = $true
        $list.BeginUpdate()
        $list.Items.Clear()
        $delete.Enabled = $false
        foreach ($connection in @($data.connections)) {
            $item = New-Object Windows.Forms.ListViewItem([string]$connection.name)
            [void]$item.SubItems.Add($(if ($connection.ignored) { 'Ignored' } elseif ($connection.active) { 'Last active' } else { 'Previously seen' }))
            $suggestion = if ($connection.existing_customers.Count) { $(if ($connection.assigned) {'Assigned: '} else {'Suggested: '}) + ($connection.existing_customers -join ', ') } else { [string]$connection.suggested_name }
            [void]$item.SubItems.Add($suggestion)
            [void]$item.SubItems.Add([string]$connection.id)
            $item.Tag = $connection
            [void]$list.Items.Add($item)
            if ($connection.id -eq $selected) { $item.Selected = $true }
        }
        $list.EndUpdate()
        $text = $name.Text
        $name.Items.Clear()
        foreach ($customer in @($data.customers)) { [void]$name.Items.Add([string]$customer) }
        $name.Text = $text
        $status.Text = "$($list.Items.Count) connection(s)"
    } catch { $status.Text = 'Refresh failed: ' + $_.Exception.Message }
    finally { $script:refreshing = $false }
}
$list.Add_SelectedIndexChanged({
    if ($script:refreshing) { return }
    if ($list.SelectedItems.Count -eq 0) { return }
    $connection = $list.SelectedItems[0].Tag
    $delete.Enabled = [bool]$connection.assigned
    $save.Text = if ($connection.assigned) { 'Rename customer' } else { 'Add connection' }
    $ignore.Text = if ($connection.ignored) { 'Restore selected' } else { 'Ignore selected' }
    $name.Text = if ($connection.existing_customers.Count -eq 1) { [string]$connection.existing_customers[0] } else { [string]$connection.suggested_name }
})
$refresh.Add_Click({ Refresh-Connections })
$ignore.Add_Click({
    if ($list.SelectedItems.Count -eq 0) { $status.Text = 'Select a connection to ignore.'; return }
    try {
        $command = if ($list.SelectedItems[0].Tag.ignored) { 'unignore' } else { 'ignore' }
        $output = & $watcherExe $command $configPath --name ([string]$list.SelectedItems[0].Tag.name) 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        Refresh-Connections
        $status.Text = if ($command -eq 'unignore') { 'Restored. The watcher will reload this connection.' } else { 'Ignored. Customer events and naming notifications exclude this VPN.' }
    } catch {
        if ($managementTestOutput) { throw }
        [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Could not ignore VPN', 'OK', 'Error')
    }
})
$ignoredSettings.Add_Click({
    $dialog = New-Object Windows.Forms.Form
    $dialog.Text = 'Ignored VPNs'
    $dialog.Size = New-Object Drawing.Size(500, 180)
    $dialog.StartPosition = 'CenterParent'
    $dialog.FormBorderStyle = 'FixedDialog'
    $dialog.MaximizeBox = $false
    $hint = New-Object Windows.Forms.Label
    $hint.Text = 'Enter the exact connection name (e.g. Polcom), or select one to restore.'
    $hint.Location = New-Object Drawing.Point(12, 12)
    $hint.Size = New-Object Drawing.Size(465, 35)
    $connectionName = New-Object Windows.Forms.ComboBox
    $connectionName.Location = New-Object Drawing.Point(12, 48)
    $connectionName.Size = New-Object Drawing.Size(460, 26)
    $connectionName.DropDownStyle = 'DropDown'
    foreach ($entry in $script:ignoredConnections) { [void]$connectionName.Items.Add([string]$entry) }
    $addIgnore = New-Object Windows.Forms.Button
    $addIgnore.Text = 'Ignore'
    $addIgnore.Location = New-Object Drawing.Point(12, 85)
    $restore = New-Object Windows.Forms.Button
    $restore.Text = 'Restore'
    $restore.Location = New-Object Drawing.Point(100, 85)
    $changeIgnore = {
        param($ignored)
        if (-not $connectionName.Text.Trim()) { return }
        $command = if ($ignored) { 'ignore' } else { 'unignore' }
        $output = & $watcherExe $command $configPath --name $connectionName.Text.Trim() 2>&1
        if ($LASTEXITCODE -ne 0) { [void][Windows.Forms.MessageBox]::Show(($output -join [Environment]::NewLine), 'Could not update ignored VPNs'); return }
        Refresh-Connections
        $connectionName.Items.Clear()
        foreach ($entry in $script:ignoredConnections) { [void]$connectionName.Items.Add([string]$entry) }
    }
    $addIgnore.Add_Click({ & $changeIgnore $true })
    $restore.Add_Click({ & $changeIgnore $false })
    $dialog.Controls.AddRange(@($hint, $connectionName, $addIgnore, $restore))
    if ($managementTestOutput) {
        $dialog.Show($form)
        [Windows.Forms.Application]::DoEvents()
        $connectionName.Text = 'Synthetic VPN B'
        $addIgnore.PerformClick()
        if ($script:ignoredConnections -notcontains 'Synthetic VPN B') { throw 'Ignored VPN settings did not save' }
        $restore.PerformClick()
        if ($script:ignoredConnections -contains 'Synthetic VPN B') { throw 'Ignored VPN settings did not restore' }
        $dialog.Close()
    } else { [void]$dialog.ShowDialog($form) }
    $dialog.Dispose()
})
$payloadSettings.Add_Click({
    $dialog = New-Object Windows.Forms.Form
    $dialog.Text = 'ActivityWatch VPN payload settings'
    $dialog.Size = New-Object Drawing.Size(510, 400)
    $dialog.StartPosition = 'CenterParent'
    try {
        $output = & $watcherExe payload $configPath 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        $policy = ($output -join [Environment]::NewLine) | ConvertFrom-Json
        $hint = New-Object Windows.Forms.Label
        $hint.Text = 'Strict sends only the workspace title. Extended adds selected fields. Customer/name choices also control title content.'
        $hint.Location = New-Object Drawing.Point(12, 12)
        $hint.Size = New-Object Drawing.Size(465, 45)
        $mode = New-Object Windows.Forms.ComboBox
        $mode.DropDownStyle = 'DropDownList'
        $mode.Location = New-Object Drawing.Point(12, 60)
        [void]$mode.Items.AddRange(@('strict', 'extended'))
        $mode.SelectedItem = [string]$policy.mode
        $dialog.Controls.AddRange(@($hint, $mode))
        $choices = [ordered]@{customers='Customer labels (also in title)';connection_names='Connection names (also unknown title)';connection_ids='Connection IDs';suggested_names='DNS-derived name suggestions';schema_version='Custom schema version'}
        $checks = @{}
        $top = 100
        foreach ($key in $choices.Keys) {
            $check = New-Object Windows.Forms.CheckBox
            $check.Text = $choices[$key]
            $check.Checked = [bool]$policy.$key
            $check.Location = New-Object Drawing.Point(12, $top)
            $check.Size = New-Object Drawing.Size(460, 25)
            $dialog.Controls.Add($check)
            $checks[$key] = $check
            $top += 30
        }
        $apply = New-Object Windows.Forms.Button
        $apply.Text = 'Save'
        $apply.Location = New-Object Drawing.Point(12, 280)
        $apply.Add_Click({
            try {
                $value = @{mode=[string]$mode.SelectedItem}
                foreach ($key in $choices.Keys) { $value[$key] = $checks[$key].Checked }
                $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($value | ConvertTo-Json -Compress)))
                $output = & $watcherExe payload $configPath --base64 $encoded 2>&1
                if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
                $dialog.Close()
            } catch { [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Could not save payload settings') }
        })
        $dialog.Controls.Add($apply)
        [void]$dialog.ShowDialog($form)
    } catch { [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Could not open payload settings') }
    finally { $dialog.Dispose() }
})
$delete.Add_Click({
    if ($list.SelectedItems.Count -eq 0) { return }
    $row = $list.SelectedItems[0].Tag
    if (-not $row.assigned) { return }
    $message = "Delete saved connection '$($row.name)'? Other connections and recorded AW history remain. If this VPN is still active, it may be discovered again as unnamed. Use Ignore to suppress future detection."
    if ([Windows.Forms.MessageBox]::Show($message, 'Delete connection', 'YesNo', 'Warning', 'Button2') -ne 'Yes') { return }
    try {
        $output = & $watcherExe delete $configPath ([string]$row.id).Substring(8) 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        Refresh-Connections
        $status.Text = 'Deleted saved connection. Recorded history is preserved.'
    } catch {
        Refresh-Connections
        [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Could not complete deletion', 'OK', 'Error')
    }
})
$save.Add_Click({
    if ($list.SelectedItems.Count -eq 0 -or -not $name.Text.Trim()) { $status.Text = 'Select a connection and enter a name.'; return }
    $row=$list.SelectedItems[0].Tag
    if($row.assigned) { $arguments=@('rename',$configPath,([string]$row.id).Substring(8),'--name',$name.Text.Trim()) }
    elseif($row.ignored) { $status.Text='Restore this connection before naming.'; return }
    else { $arguments=@('name',$configPath,[string]$row.id,'--name',$name.Text.Trim()) }
    try {
        $output = & $watcherExe @arguments 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        Refresh-Connections
        $status.Text = 'Saved. Existing customer connections and history are preserved.'
    } catch {
        if ($managementTestOutput) { throw }
        [void][Windows.Forms.MessageBox]::Show(($_.Exception.Message), 'Could not save name', 'OK', 'Error')
    }
})
$timer = New-Object Windows.Forms.Timer
$timer.Interval = 5000
$timer.Add_Tick({
    if ($watcherPid -and -not (Get-Process -Id $watcherPid -ErrorAction SilentlyContinue)) { $form.Close(); return }
    Refresh-Connections
})
$timer.Start()
$tray = New-Object Windows.Forms.NotifyIcon
$tray.Icon = [Drawing.SystemIcons]::Information
$tray.Text = 'ActivityWatch VPN - unnamed connections'
$tray.Visible = $true
$tray.Add_BalloonTipClicked({ $form.Show(); $form.Activate(); Refresh-Connections })
$tray.Add_DoubleClick({ $form.Show(); $form.Activate(); Refresh-Connections })
$menu = New-Object Windows.Forms.ContextMenuStrip
[void]$menu.Items.Add('Name connections', $null, { $form.Show(); $form.Activate(); Refresh-Connections })
[void]$menu.Items.Add('Close naming window', $null, { $form.Close() })
$tray.ContextMenuStrip = $menu
$form.Add_Shown({
    Refresh-Connections
    if ($notificationMessage) {
        $form.Hide()
        if ($script:notifyEnabled) { $tray.ShowBalloonTip(10000, 'ActivityWatch VPN', 'VPN connections detected. Click to name them.', [Windows.Forms.ToolTipIcon]::Info) }
    }
})
try {
    if ($managementTestOutput) {
        # Test harness supplies synthetic paths; never used by the CLI bootstrap.
        $form.Show()
        [Windows.Forms.Application]::DoEvents()
        Refresh-Connections
        if ($list.Items.Count -ne 2) { throw "Expected two unnamed connections; $($status.Text)" }
        $form.PerformLayout()
        $image = New-Object Drawing.Bitmap($form.Width, $form.Height)
        $form.DrawToBitmap($image, (New-Object Drawing.Rectangle(0, 0, $form.Width, $form.Height)))
        $image.Save($managementTestOutput, [Drawing.Imaging.ImageFormat]::Png)
        $image.Dispose()
        $list.Items[0].Selected = $true
        [Windows.Forms.Application]::DoEvents()
        $name.Text = 'Synthetic Customer A'
        $save.PerformClick()
        if ($list.Items.Count -ne 2 -or @($list.Items | Where-Object {$_.Tag.assigned}).Count -ne 1) { throw "Named connection disappeared; $($status.Text)" }
        ($list.Items | Where-Object {$_.Tag.assigned}).Selected=$true
        [Windows.Forms.Application]::DoEvents()
        $name.Text='Synthetic Customer Renamed'
        $save.PerformClick()
        if (@($list.Items | Where-Object {$_.Tag.existing_customers -contains 'Synthetic Customer Renamed'}).Count -ne 1) { throw 'Assigned connection could not be renamed' }
        ($list.Items | Where-Object {$_.Tag.name -eq 'Synthetic VPN B'}).Selected = $true
        [Windows.Forms.Application]::DoEvents()
        $ignore.PerformClick()
        if ($list.Items.Count -ne 2 -or @($list.Items | Where-Object {$_.Tag.ignored}).Count -ne 1) { throw 'Ignored status is not visible' }
        $image=New-Object Drawing.Bitmap($form.Width,$form.Height)
        $form.DrawToBitmap($image,(New-Object Drawing.Rectangle(0,0,$form.Width,$form.Height)))
        $image.Save(($managementTestOutput+'.ignored.png'),[Drawing.Imaging.ImageFormat]::Png)
        $image.Dispose()
        ($list.Items | Where-Object {$_.Tag.ignored}).Selected = $true
        [Windows.Forms.Application]::DoEvents()
        $ignore.PerformClick()
        Refresh-Connections
        if ($list.Items.Count -ne 2 -or @($list.Items | Where-Object {$_.Tag.ignored}).Count) { throw 'Restoring did not clear ignored status' }
        $ignoredSettings.PerformClick()
        'UI listing, naming, ignore and restore checks passed'
    } else { [Windows.Forms.Application]::Run($form) }
}
finally { $timer.Dispose(); $tray.Dispose(); $form.Dispose(); $mutex.ReleaseMutex(); $mutex.Dispose() }
