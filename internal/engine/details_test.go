package engine

import (
	"context"
	"io/fs"
	"slices"
	"strconv"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// fakeStat is a disk that knows some paths, and records which it was asked.
type fakeStat struct {
	infos map[string]fakeInfo
	asked []string
}

type fakeInfo struct {
	size  int64
	mod   time.Time
	isDir bool
}

func (f *fakeStat) Lstat(path string) (fs.FileInfo, error) {
	f.asked = append(f.asked, path)
	info, ok := f.infos[path]
	if !ok {
		return nil, fs.ErrNotExist
	}
	return info, nil
}

func (i fakeInfo) Name() string       { return "" }
func (i fakeInfo) Size() int64        { return i.size }
func (i fakeInfo) Mode() fs.FileMode  { return map[bool]fs.FileMode{true: fs.ModeDir}[i.isDir] }
func (i fakeInfo) ModTime() time.Time { return i.mod }
func (i fakeInfo) IsDir() bool        { return i.isDir }
func (i fakeInfo) Sys() any           { return nil }

// Ports preview.rs: details only for what TinyDash indexed or offered, so a
// view cannot probe other paths.
func TestDetailsOnlyOfOfferedItems(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, db *store.Store) {
		withLibrary(first, db)
		first.Settings.ClipboardHistoryEnabled = true
	})
	notes := golden.Abs("/Users/me/notes.txt")
	modified := time.Unix(1_700_000_000, 0)
	disk := &fakeStat{infos: map[string]fakeInfo{
		safari: {size: 4096, mod: modified, isDir: true},
		notes:  {size: 5, mod: modified},
	}}

	if _, ok := w.actions.Details(core.SourceFile.ID(notes), disk); ok || len(disk.asked) != 0 {
		t.Errorf("a file not offered has details, or its path was read: %v", disk.asked)
	}
	w.actions.RememberFound(notes)
	got, ok := w.actions.Details(core.SourceFile.ID(notes), disk)
	if want := (Details{Kind: DetailsFile, File: FileFacts{Path: notes, Size: 5, Modified: modified}}); !ok || !sameDetails(got, want) {
		t.Errorf("file details %+v, want %+v", got, want)
	}
	got, ok = w.actions.Details(core.SourceApp.ID(safari), disk)
	if want := (Details{Kind: DetailsFile, File: FileFacts{Path: safari, Size: 4096, Modified: modified, IsDir: true}}); !ok || !sameDetails(got, want) {
		t.Errorf("app details %+v, want %+v", got, want)
	}
	if _, ok := w.actions.Details(core.SourceApp.ID(golden.Abs("/Applications/Gone.app")), disk); ok {
		t.Error("an app not indexed has details")
	}

	got, ok = w.actions.Details("snippet:1", disk)
	if want := (Details{Kind: DetailsText, Text: "Regards, {clipboard}"}); !ok || !sameDetails(got, want) {
		t.Errorf("snippet details %+v, want %+v", got, want)
	}
	for _, id := range []core.ID{"snippet:2", "link:2", "snippet:99", "clip:abc", "clip:99", "nothing:abc", "system:lock"} {
		if got, ok := w.actions.Details(id, disk); ok {
			t.Errorf("%s has details %+v", id, got)
		}
	}
}

func TestDetailsOfClipboardEntriesWhileHistoryIsOn(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Settings.ClipboardHistoryEnabled = true
	})
	report := golden.Abs("/Users/me/report.pdf")
	for _, content := range []clipboard.Content{
		clipboard.Text("hello\nworld"),
		clipboard.Files([]string{report, home}),
		clipboard.Image([]byte{1, 2, 3}, 4, 5),
	} {
		if err := <-w.engine.SaveClip(w.engine.ClipEpoch(), content, testNow.Unix()); err != nil {
			t.Fatal(err)
		}
	}
	entries, err := w.db.ClipboardHistory(context.Background())
	if err != nil || len(entries) != 3 {
		t.Fatalf("%v %v", entries, err)
	}
	id := func(n int64) core.ID { return core.SourceClip.ID(strconv.FormatInt(n, 10)) }
	disk := &fakeStat{}
	for _, c := range []struct {
		id   core.ID
		want Details
	}{
		{id(entries[2].ID), Details{Kind: DetailsText, Text: "hello\nworld"}},
		{id(entries[1].ID), Details{Kind: DetailsFiles, Paths: []string{report, home}}},
		{id(entries[0].ID), Details{Kind: DetailsImage, PNG: []byte{1, 2, 3}, Width: 4, Height: 5}},
	} {
		if got, ok := w.actions.Details(c.id, disk); !ok || !sameDetails(got, c.want) {
			t.Errorf("%s: %+v, want %+v", c.id, got, c.want)
		}
	}
	w.edit(func(next *search.Snapshot) { next.Settings.ClipboardHistoryEnabled = false })
	if got, ok := w.actions.Details(id(entries[2].ID), disk); ok {
		t.Errorf("details %+v while history is off", got)
	}
	if len(disk.asked) != 0 {
		t.Errorf("read the disk for clipboard entries: %v", disk.asked)
	}
}

func TestNoDetailsOfAFileTheDiskCannotRead(t *testing.T) {
	w := newWorld(t, nil)
	if _, ok := w.actions.Details(core.SourceApp.ID(safari), &fakeStat{}); ok {
		t.Error("details of an app the disk does not have")
	}
}

func sameDetails(a, b Details) bool {
	return a.Kind == b.Kind && a.Text == b.Text && slices.Equal(a.PNG, b.PNG) && a.Width == b.Width && a.Height == b.Height &&
		slices.Equal(a.Paths, b.Paths) && a.File.Path == b.File.Path && a.File.Size == b.File.Size &&
		a.File.Modified.Equal(b.File.Modified) && a.File.IsDir == b.File.IsDir
}
