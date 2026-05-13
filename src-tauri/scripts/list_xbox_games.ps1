# LICENSE: AGPLv3. See LICENSE at root of repo

$targets = get-AppxPackage
$targets = $targets.where{ -not $_.IsFramework }

$apps = @()
foreach ($app in $targets)
{
    try
    {
        $app_manifest = Get-AppxPackageManifest $app;
        $name = $app_manifest.Package.Properties.DisplayName;
        if ($name -like '*DisplayName*' -or $name -like '*ms-resource*')
        {
            # Invalid name is probably not a game.
            continue;
        }

        # When a package has multiple applications, the manifest fields are
        # arrays. Take the first application entry so we always get a scalar.
        $application = $app_manifest.package.applications.application;
        if ($application -is [array]) { $application = $application[0] }

        # Lots of games use $id = Game. Older games (like Prey) are App. Some
        # games use nonsense (Supraland, GenesisNoir). So we can't exclude
        # based on id, but we can include if it's 'Game'.
        $id = $application.id;

        # Small icon looks better in steam. The Square150x150Logo is better for a desktop shortcut.
        $icon = $app.InstallLocation + "\" + $application.VisualElements.Square44x44Logo;
        $apps += [pscustomobject]@{
            Kind = $id
            Appid = $app.Name
            PrettyName = $name
            Icon = $icon
            InstallLocation = $app.InstallLocation
            Aumid = $app.PackageFamilyName + "!" + $id
        };
    }
    catch
    {
    }
}

$apps | ConvertTo-Json -depth 100;
