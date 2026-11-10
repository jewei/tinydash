package kit

import "runtime"

// SuperName is what the desktop calls the Super key: Windows calls it Win.
var SuperName = map[bool]string{true: "Win", false: "Super"}[runtime.GOOS == "windows"]
