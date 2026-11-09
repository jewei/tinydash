package platform

// Volume is the disk that holds a folder.
type Volume struct {
	// Name is the Finder name on macOS, the drive on Windows (C:), and the
	// mount point on Linux (/).
	Name       string
	TotalBytes uint64
	// FreeBytes is space left to fill. On macOS it counts purgeable space,
	// as Finder does.
	FreeBytes uint64
}
