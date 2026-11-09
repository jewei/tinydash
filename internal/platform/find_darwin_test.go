package platform

import (
	"context"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"
)

func all(string) bool { return true }

func TestReadsAtMostTheLimit(t *testing.T) {
	paths := readPaths(context.Background(), strings.NewReader("/a\n/b\n/c\n"), 2, time.Second, all)
	if want := []string{"/a", "/b"}; !slices.Equal(paths, want) {
		t.Errorf("got %q, want %q", paths, want)
	}
}

func TestTheLimitCountsOnlyThePathsKept(t *testing.T) {
	keep := func(path string) bool { return nameContains(path, "-draft") }
	paths := readPaths(context.Background(), strings.NewReader("/draft.txt\n/Drafts\n/-DRAFT.txt\n"), 2, time.Second, keep)
	if want := []string{"/-DRAFT.txt"}; !slices.Equal(paths, want) {
		t.Errorf("got %q, want %q", paths, want)
	}
}

// A search that goes on past the timeout keeps the paths it gave so far.
func TestKeepsThePathsFoundBeforeTheTimeout(t *testing.T) {
	r, w := io.Pipe()
	defer w.Close()
	go func() { _, _ = io.WriteString(w, "/a\n") }()
	paths := readPaths(context.Background(), r, 10, 100*time.Millisecond, all)
	if want := []string{"/a"}; !slices.Equal(paths, want) {
		t.Errorf("got %q, want %q", paths, want)
	}
}

// perl, like mdfind, holds its output in a buffer when it writes into a
// pipe, and writes each line at once onto a terminal.
func TestATerminalGivesEachLineAsItIsWritten(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	command := exec.CommandContext(ctx, "/usr/bin/perl", "-e", `print "/a\n"; sleep 5`)
	output, err := startOnTerminal(command)
	if err != nil {
		t.Fatal(err)
	}
	defer output.Close()
	paths := readPaths(ctx, output, 10, time.Second, all)
	cancel()
	_ = command.Wait()
	if want := []string{"/a"}; !slices.Equal(paths, want) {
		t.Errorf("got %q, want %q", paths, want)
	}
}

func TestANameOfDashesAsksNothing(t *testing.T) {
	if paths := FindFiles(context.Background(), "--", "/", 10); paths != nil {
		t.Errorf("got %q", paths)
	}
}

// Runs Spotlight for real, on this checkout, which Spotlight indexes as
// part of the user's home: TINYDASH_LIVE_SPOTLIGHT=1 go test ./internal/platform -run Live.
func TestLiveSpotlightFindsAFileByName(t *testing.T) {
	if os.Getenv("TINYDASH_LIVE_SPOTLIGHT") == "" {
		t.Skip("set TINYDASH_LIVE_SPOTLIGHT=1 to ask Spotlight")
	}
	folder, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	paths := FindFiles(context.Background(), "go-architecture", folder, 5)
	if want := filepath.Join(folder, "docs", "go-architecture.md"); !slices.Contains(paths, want) {
		t.Errorf("got %q, want %q among them", paths, want)
	}
}
