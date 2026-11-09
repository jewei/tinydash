package platform

import (
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"testing"
)

func TestEmptyXDGVariablesUseTheDefaults(t *testing.T) {
	defaults := []string{"/home/me/.local/share/applications", "/usr/local/share/applications", "/usr/share/applications"}
	if got := xdgAppFolders("/home/me", "", ""); !reflect.DeepEqual(got, defaults) {
		t.Errorf("unset: %v", got)
	}
	if got := xdgAppFolders("/home/me", "/data", "/opt/share:"); !reflect.DeepEqual(got, []string{"/data/applications", "/opt/share/applications"}) {
		t.Errorf("set: %v", got)
	}
}

func TestAppFoldersIncludeFlatpakAndSnapExports(t *testing.T) {
	got := xdgAppFolders("/home/ann", "/data", "/var/lib/flatpak/exports/share::/var/lib/snapd/desktop:")
	want := []string{"/data/applications", "/var/lib/flatpak/exports/share/applications", "/var/lib/snapd/desktop/applications"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("got %v, want %v", got, want)
	}
}

// installed is a canRun that finds the programs named, by path or name.
func installed(programs ...string) func(string) bool {
	return func(program string) bool { return slices.Contains(programs, program) }
}

func TestLauncherWrappersAreNotAliases(t *testing.T) {
	env := desktopEnv{canRun: installed("/usr/bin/flatpak", "env", "/usr/bin/snap", "/usr/bin/firefox", "bash")}
	for exec, want := range map[string][]string{
		"/usr/bin/flatpak run --command=x org.App": nil,
		"env BAMF_DESKTOP_FILE_HINT=x /snap/bin/x": nil,
		"/usr/bin/snap run x":                      nil,
		"bash -c x":                                nil,
		"/usr/bin/firefox %u":                      {"firefox"},
		"":                                         nil,
	} {
		app, ok := desktopApp("a.desktop", "/a.desktop", "[Desktop Entry]\nType=Application\nName=A\nExec="+exec, env)
		if !ok || !reflect.DeepEqual(app.Aliases, want) {
			t.Errorf("Exec=%s: aliases %q (%v), want %q", exec, app.Aliases, ok, want)
		}
	}
}

func TestReadsADesktopEntryAsGIODid(t *testing.T) {
	env := desktopEnv{languages: languageNames(func(name string) string {
		return map[string]string{"LANG": "de_DE.UTF-8"}[name]
	}), desktops: []string{"ubuntu", "GNOME"}, canRun: installed("firefox", "/opt/tool/tool", "true")}
	base := "[Desktop Entry]\nType=Application\nName=App\nExec=true\n"
	for _, c := range []struct {
		name, text string
		want       *App
	}{
		{"plain", base, &App{Name: "App", Path: "/a.desktop", Aliases: []string{"true"}}},
		{"localized name, keywords, and generic name",
			"[Desktop Entry]\nType=Application\nName=Firefox Web Browser\nName[de]=Firefox-Webbrowser\nGenericName=Web Browser\nGenericName[de_DE]=Webbrowser\nComment=Browse the Web\nKeywords=Internet;WWW;\nKeywords[de]=Internet;Netz\\;Web;\nExec=firefox %u\n",
			&App{Name: "Firefox-Webbrowser", Path: "/a.desktop", Aliases: []string{"Internet", "Netz;Web", "firefox"}, Description: "Webbrowser"}},
		{"full name first", base + "X-GNOME-FullName=App Suite\n", &App{Name: "App Suite", Path: "/a.desktop", Aliases: []string{"true"}}},
		{"comment without a generic name", base + "Comment=Does\\sthings\n", &App{Name: "App", Path: "/a.desktop", Aliases: []string{"true"}, Description: "Does things"}},
		{"a blank generic name hides the comment", base + "GenericName= \nComment=Does things\n", &App{Name: "App", Path: "/a.desktop", Aliases: []string{"true"}}},
		{"quoted program", "[Desktop Entry]\nType=Application\nName=Tool\nExec=\"/opt/tool/tool\" --new\n", &App{Name: "Tool", Path: "/a.desktop", Aliases: []string{"tool"}}},
		{"only this desktop", base + "OnlyShowIn=GNOME;\n", &App{Name: "App", Path: "/a.desktop", Aliases: []string{"true"}}},
		{"keys in another group", base + "[Desktop Action new]\nName=Other\nNoDisplay=true\n", &App{Name: "App", Path: "/a.desktop", Aliases: []string{"true"}}},
		{"not an application", "[Desktop Entry]\nType=Link\nName=App\nURL=https://example.com\n", nil},
		{"no display", base + "NoDisplay=true\n", nil},
		{"hidden", base + "Hidden=true\n", nil},
		{"only another desktop", base + "OnlyShowIn=KDE;XFCE;\n", nil},
		{"not this desktop", base + "NotShowIn=ubuntu;\n", nil},
		{"TryExec not installed", base + "TryExec=missing\n", nil},
		{"Exec not installed", "[Desktop Entry]\nType=Application\nName=App\nExec=missing --flag\n", nil},
		{"Exec with a quote that does not close", "[Desktop Entry]\nType=Application\nName=App\nExec=\"firefox\n", nil},
		{"no name", "[Desktop Entry]\nType=Application\nExec=true\n", nil},
	} {
		got, ok := desktopApp("a.desktop", "/a.desktop", c.text, env)
		if c.want == nil {
			if ok {
				t.Errorf("%s: listed as %+v", c.name, got)
			}
			continue
		}
		if !ok || !reflect.DeepEqual(got, *c.want) {
			t.Errorf("%s: got %+v (%v), want %+v", c.name, got, ok, *c.want)
		}
	}
	if _, ok := desktopApp("tinydash.desktop", "/tinydash.desktop", base, env); ok {
		t.Error("TinyDash lists itself")
	}
}

func TestFirstArgumentReadsShellQuoting(t *testing.T) {
	for line, want := range map[string]string{
		"firefox %u":                   "firefox",
		"  /usr/bin/code --new-window": "/usr/bin/code",
		`"/opt/My App/app" %F`:         "/opt/My App/app",
		`'/opt/My App/app'`:            "/opt/My App/app",
		`/opt/My\ App/app`:             "/opt/My App/app",
		`"say \"hi\""`:                 `say "hi"`,
		`"a\b"`:                        `a\b`,
		`env"x"y z`:                    "envxy",
		"":                             "",
		`"open`:                        "",
	} {
		got, ok := firstArgument(line)
		if got != want || ok != (want != "") {
			t.Errorf("firstArgument(%q) = %q, %v; want %q", line, got, ok, want)
		}
	}
}

func TestLanguageNamesFollowGLib(t *testing.T) {
	for _, c := range []struct {
		env  map[string]string
		want []string
	}{
		{map[string]string{"LANG": "en_GB.UTF-8@euro"}, []string{"en_GB.UTF-8@euro", "en_GB.UTF-8", "en_GB@euro", "en_GB", "en.UTF-8@euro", "en.UTF-8", "en@euro", "en"}},
		{map[string]string{"LANG": "fr_BE", "LC_MESSAGES": "de"}, []string{"de"}},
		{map[string]string{"LANGUAGE": "pt_BR:pt", "LC_ALL": "de_DE"}, []string{"pt_BR", "pt"}},
		{map[string]string{"LANG": "C.UTF-8"}, nil},
		{nil, nil},
	} {
		if got := languageNames(func(name string) string { return c.env[name] }); !reflect.DeepEqual(got, c.want) {
			t.Errorf("%v: %q, want %q", c.env, got, c.want)
		}
	}
}

func write(t *testing.T, path, text string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

// The desktop entries of fixture folders become apps of the shape of the
// search golden world (internal/search/search_golden_test.go), aliases and
// descriptions included. The user's folder comes first, and an entry
// there, even a hidden one, hides the system's entry of the same ID.
func TestDiscoversDesktopEntriesAsGIODid(t *testing.T) {
	root := t.TempDir()
	user, system := filepath.Join(root, "home", "applications"), filepath.Join(root, "usr", "applications")
	entry := func(name, extra string) string {
		return "[Desktop Entry]\nType=Application\nName=" + name + "\n" + extra
	}
	write(t, filepath.Join(system, "code.desktop"), entry("Visual Studio Code", "GenericName=Code editor\nExec=/usr/share/code/code %F\n"))
	write(t, filepath.Join(system, "slack.desktop"), entry("Slack", "Comment=Team chat\nExec=/usr/bin/slack %U\n"))
	write(t, filepath.Join(user, "slack.desktop"), entry("Slack (mine)", "Exec=/usr/bin/slack\n"))
	write(t, filepath.Join(system, "old.desktop"), entry("Old", "Exec=/usr/bin/old\n"))
	write(t, filepath.Join(user, "old.desktop"), entry("Old", "Hidden=true\n"))
	write(t, filepath.Join(system, "kde4", "tool.desktop"), entry("KDE Tool", ""))
	write(t, filepath.Join(user, "kde4-tool.desktop"), entry("Tool, mine", ""))
	write(t, filepath.Join(system, "TinyDash.desktop"), entry("TinyDash", ""))
	write(t, filepath.Join(system, "notes.txt"), entry("Not an entry", ""))
	env := desktopEnv{canRun: installed("/usr/share/code/code", "/usr/bin/slack", "/usr/bin/old")}

	got := discoverApps([]string{user, system, filepath.Join(root, "missing")}, env)
	want := []App{
		{Name: "Tool, mine", Path: filepath.Join(user, "kde4-tool.desktop")},
		{Name: "Slack (mine)", Path: filepath.Join(user, "slack.desktop"), Aliases: []string{"slack"}},
		{Name: "Visual Studio Code", Path: filepath.Join(system, "code.desktop"), Aliases: []string{"code"}, Description: "Code editor"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("found\n%+v\nwant\n%+v", got, want)
	}
}

func TestATooLongFileIsNotADesktopEntry(t *testing.T) {
	folder := t.TempDir()
	write(t, filepath.Join(folder, "big.desktop"), "[Desktop Entry]\nType=Application\nName=Big\n#"+string(make([]byte, maxDesktopEntryBytes)))
	if got := discoverApps([]string{folder}, desktopEnv{}); len(got) != 0 {
		t.Errorf("listed %+v", got)
	}
}
