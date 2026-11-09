package platform

// App is an installed app as TinyDash describes it, in the shape of
// apps.App: the platform cannot import the feature packages.
type App struct {
	Name string
	Path string
	// Aliases are other names that find the app, such as its executable.
	Aliases []string
	// Description says what the app is for; it is empty when the OS
	// cannot say.
	Description string
}
