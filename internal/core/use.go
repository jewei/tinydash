package core

// Use is how often and how lately a result ran, for ranking.
type Use struct {
	Count    uint32
	LastUsed int64 // Unix seconds
}
