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
    $window = [VpnManagerWindow]::FindWindow($null, "ActivityWatch - Unnamed VPN connections ($configPath)")
    if ($window -ne [IntPtr]::Zero -and -not $notificationMessage) {
        [void][VpnManagerWindow]::ShowWindow($window, 5)
        [void][VpnManagerWindow]::SetForegroundWindow($window)
    }
    $mutex.Dispose()
    exit
}
$form = New-Object Windows.Forms.Form
$form.Text = "ActivityWatch - Unnamed VPN connections ($configPath)"
$form.Size = New-Object Drawing.Size(820, 510)
$form.MinimumSize = New-Object Drawing.Size(680, 440)
$form.StartPosition = 'CenterScreen'
$form.Padding = New-Object Windows.Forms.Padding(12)
$layout = New-Object Windows.Forms.TableLayoutPanel
$layout.Dock = 'Fill'
$layout.ColumnCount = 1
$layout.RowCount = 6
foreach ($height in @(38, 0, 24, 32, 32, 40)) {
    $style = New-Object Windows.Forms.RowStyle
    if ($height -eq 0) { $style.SizeType = 'Percent'; $style.Height = 100 }
    else { $style.SizeType = 'Absolute'; $style.Height = $height }
    [void]$layout.RowStyles.Add($style)
}
$form.Controls.Add($layout)
$intro = New-Object Windows.Forms.Label
$intro.Text = 'Select an unnamed connection, then enter a customer name or select an existing customer.'
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
[void]$list.Columns.Add('Connection ID', 300)
$layout.Controls.Add($list, 0, 1)
$label = New-Object Windows.Forms.Label
$label.Text = 'Customer name'
$label.Dock = 'Fill'
$layout.Controls.Add($label, 0, 2)
$name = New-Object Windows.Forms.ComboBox
$name.Dock = 'Fill'
$name.DropDownStyle = 'DropDown'
$layout.Controls.Add($name, 0, 3)
$replace = New-Object Windows.Forms.CheckBox
$replace.Text = 'Update an existing customer fingerprint (preserves its ID and history)'
$replace.Dock = 'Fill'
$layout.Controls.Add($replace, 0, 4)
$buttons = New-Object Windows.Forms.FlowLayoutPanel
$buttons.Dock = 'Fill'
$save = New-Object Windows.Forms.Button
$save.Text = 'Save name'
$save.AutoSize = $true
$refresh = New-Object Windows.Forms.Button
$refresh.Text = 'Refresh'
$refresh.AutoSize = $true
$status = New-Object Windows.Forms.Label
$status.AutoSize = $true
$status.Padding = New-Object Windows.Forms.Padding(8)
$buttons.Controls.AddRange(@($save, $refresh, $status))
$layout.Controls.Add($buttons, 0, 5)

function Refresh-Connections {
    try {
        $script:refreshing = $true
        $selected = if ($list.SelectedItems.Count) { $list.SelectedItems[0].Tag.id } else { $null }
        $output = & $watcherExe unnamed $configPath 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        $data = ($output -join [Environment]::NewLine) | ConvertFrom-Json
        $ids = @($data.connections | ForEach-Object id)
        if ($notificationMessage -and $script:loaded -and @($ids | Where-Object { $script:previousIds -notcontains $_ }).Count -gt 0) {
            $tray.ShowBalloonTip(10000, 'ActivityWatch VPN', 'New unnamed VPN connections detected. Click to name them.', [Windows.Forms.ToolTipIcon]::Info)
        }
        $script:previousIds = $ids
        $script:loaded = $true
        $list.BeginUpdate()
        $list.Items.Clear()
        foreach ($connection in @($data.connections)) {
            $item = New-Object Windows.Forms.ListViewItem([string]$connection.name)
            [void]$item.SubItems.Add($(if ($connection.active) { 'Last active' } else { 'Previously seen' }))
            $suggestion = if ($connection.existing_customers.Count) { 'Existing suggestion: ' + ($connection.existing_customers -join ', ') } else { [string]$connection.suggested_name }
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
        $status.Text = "$($list.Items.Count) unnamed connection(s)"
    } catch { $status.Text = 'Refresh failed: ' + $_.Exception.Message }
    finally { $script:refreshing = $false }
}
$list.Add_SelectedIndexChanged({
    if ($script:refreshing) { return }
    if ($list.SelectedItems.Count -eq 0) { return }
    $connection = $list.SelectedItems[0].Tag
    $replace.Checked = $false
    $name.Text = if ($connection.existing_customers.Count -eq 1) { [string]$connection.existing_customers[0] } else { [string]$connection.suggested_name }
})
$refresh.Add_Click({ Refresh-Connections })
$save.Add_Click({
    if ($list.SelectedItems.Count -eq 0 -or -not $name.Text.Trim()) { $status.Text = 'Select a connection and enter a name.'; return }
    $arguments = @('name', $configPath, [string]$list.SelectedItems[0].Tag.id, '--name', $name.Text.Trim())
    if ($replace.Checked) {
        $answer = [Windows.Forms.MessageBox]::Show('Replace the selected customer fingerprint with this connection? Other customer profiles and history are preserved.', 'Update fingerprint', 'YesNo', 'Question')
        if ($answer -ne 'Yes') { return }
        $arguments += '--replace'
    }
    try {
        $output = & $watcherExe @arguments 2>&1
        if ($LASTEXITCODE -ne 0) { throw ($output -join [Environment]::NewLine) }
        Refresh-Connections
        $status.Text = 'Saved. The watcher will reload the customer name.'
    } catch {
        if ($managementTestOutput) { throw }
        [void][Windows.Forms.MessageBox]::Show(($_.Exception.Message), 'Could not save name', 'OK', 'Error')
    }
})
$timer = New-Object Windows.Forms.Timer
$timer.Interval = 5000
$timer.Add_Tick({ Refresh-Connections })
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
        $tray.ShowBalloonTip(10000, 'ActivityWatch VPN', 'Unnamed VPN connections detected. Click to name them.', [Windows.Forms.ToolTipIcon]::Info)
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
        if ($list.Items.Count -ne 1) { throw "Naming did not remove the resolved connection; $($status.Text)" }
        'UI list and naming checks passed'
    } else { [Windows.Forms.Application]::Run($form) }
}
finally { $timer.Dispose(); $tray.Dispose(); $form.Dispose(); $mutex.ReleaseMutex(); $mutex.Dispose() }
