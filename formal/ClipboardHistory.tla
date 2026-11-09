---------------------------- MODULE ClipboardHistory ----------------------------
(***************************************************************************)
(* The clipboard history of TinyDash: the monitor that captures what the   *)
(* user copies, and the engine's single writer that saves it, clears it,   *)
(* and switches history off and on.                                        *)
(*                                                                         *)
(* Code (internal/engine):                                                 *)
(*   MonitorStarts, MonitorFinishes, MonitorQueuesSave                     *)
(*                        = Monitor.Tick (monitor.go)                      *)
(*   WriterApplies "save" = Engine.SaveClip (clipboard.go)                 *)
(*   WriterApplies "clear"= Engine.ClearClipboard (clipboard.go)           *)
(*   WriterApplies "off"/"on" = Engine.UpdateSettings (settings.go)        *)
(*   queue                = Engine.changes; the writer is Engine.Run        *)
(*   epoch, offs          = Engine.clipEpoch, Engine.historyOffs           *)
(*                                                                         *)
(* Constants:                                                              *)
(*   EpochFirst: Monitor.Tick reads the epoch before the clipboard (code). *)
(*   Counted: the OS has a change counter (macOS, Windows); FALSE is Linux, *)
(*     where the monitor compares content (a copy's number here, a hash in *)
(*     the code).                                                          *)
(*   Fixed: the code as it is: switch-offs counted; after history was off  *)
(*     where no counter kept up, the next read is a baseline; and a copy   *)
(*     during a read drops that read. FALSE is the code before the fixes:  *)
(*     a switch-off reset the seen content, and no recheck.                *)
(* The configs whose names hold ".broken" must fail; the others must pass. *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS EpochFirst, Counted, Fixed, MaxCopies, MaxRequests

VARIABLES
    clip,      \* what the OS clipboard holds: the number of the copy; 0 was there at startup
    gen,       \* clears and switch-offs the writer applied: real time, not the epoch
    epoch,     \* Engine.clipEpoch
    offs,      \* Engine.historyOffs
    enabled,   \* Settings.ClipboardHistoryEnabled in the published snapshot
    history,   \* saved entries: [copy, readGen, savedGen]
    cleared,   \* copies that were in the history when a Clear applied
    queue,     \* changes waiting for the writer, in order
    requests,  \* clears and switches the user asked for, to bound the model
    started,   \* Monitor.started
    seen,      \* Monitor.seen: the change counter at the last tick
    seenCopy,  \* Monitor.seenHash: the content last read (Linux compares it)
    rebase,    \* Monitor.rebase
    monOffs,   \* Monitor.offs
    mon,       \* where Monitor.Tick is: "idle", "half", "ready"
    monFirst, monEpoch, monCopy, monReadGen

world == <<clip, gen, epoch, offs, enabled, history, cleared, queue, requests>>
monVars == <<started, seen, seenCopy, rebase, monOffs, mon, monFirst, monEpoch, monCopy, monReadGen>>
vars == <<world, monVars>>

Init ==
    /\ clip = 0 /\ gen = 0 /\ epoch = 0 /\ offs = 0 /\ enabled = TRUE
    /\ history = {} /\ cleared = {} /\ queue = <<>> /\ requests = 0
    /\ started = FALSE /\ seen = 0 /\ seenCopy = 0 /\ rebase = FALSE /\ monOffs = 0
    /\ mon = "idle" /\ monFirst = FALSE /\ monEpoch = 0 /\ monCopy = 0 /\ monReadGen = 0

UserCopies ==
    /\ clip < MaxCopies
    /\ clip' = clip + 1
    /\ UNCHANGED <<gen, epoch, offs, enabled, history, cleared, queue, requests>>
    /\ UNCHANGED monVars

UserAsks(kind) ==
    /\ requests < MaxRequests
    /\ queue' = Append(queue, [kind |-> kind])
    /\ requests' = requests + 1
    /\ UNCHANGED <<clip, gen, epoch, offs, enabled, history, cleared>>
    /\ UNCHANGED monVars

\* The first step of a tick: the change counter, the switch-off count, the
\* epoch (when EpochFirst), and whether history is on, read before the
\* clipboard, as the code does.
MonitorStarts ==
    /\ mon = "idle"
    /\ LET rb == rebase \/ (Fixed /\ offs # monOffs) IN
       IF Counted /\ started /\ clip = seen THEN
            \* Nothing changed; with history on, what is there is the baseline.
            /\ rebase' = (rb /\ ~enabled)
            /\ monOffs' = offs
            /\ UNCHANGED <<started, seen, seenCopy, mon, monFirst, monEpoch, monCopy, monReadGen>>
       ELSE IF ~enabled THEN
            \* Nothing is read while history is off. With a counter, the tick
            \* took the change as seen; without one, the next read is a
            \* baseline. Before the fix, the seen content was reset.
            /\ rebase' = (Fixed /\ ~Counted)
            /\ seenCopy' = IF Fixed THEN seenCopy ELSE 0
            /\ seen' = clip /\ started' = TRUE /\ monOffs' = offs
            /\ UNCHANGED <<mon, monFirst, monEpoch, monCopy, monReadGen>>
       ELSE
            /\ monFirst' = (~started \/ rb)
            /\ rebase' = FALSE /\ seen' = clip /\ started' = TRUE /\ monOffs' = offs
            /\ IF EpochFirst THEN monEpoch' = epoch /\ UNCHANGED <<monCopy, monReadGen>>
                             ELSE monCopy' = clip /\ monReadGen' = gen /\ UNCHANGED monEpoch
            /\ mon' = "half"
            /\ UNCHANGED seenCopy
    /\ UNCHANGED world

\* The second read, then the choice: a first look is only a baseline; on
\* Linux the same content as last time is nothing new.
MonitorFinishes ==
    /\ mon = "half"
    /\ IF EpochFirst THEN monCopy' = clip /\ monReadGen' = gen /\ UNCHANGED monEpoch
                     ELSE monEpoch' = epoch /\ UNCHANGED <<monCopy, monReadGen>>
    /\ LET c == IF EpochFirst THEN clip ELSE monCopy IN
       \* The counter again: a copy during the read drops the read (Fixed).
       IF Fixed /\ Counted /\ clip # seen THEN mon' = "idle" /\ UNCHANGED seenCopy
       ELSE IF monFirst THEN seenCopy' = c /\ mon' = "idle"
       ELSE IF ~Counted /\ c = seenCopy THEN mon' = "idle" /\ UNCHANGED seenCopy
       ELSE seenCopy' = c /\ mon' = "ready"
    /\ UNCHANGED <<started, seen, rebase, monOffs, monFirst>>
    /\ UNCHANGED world

MonitorQueuesSave ==
    /\ mon = "ready"
    /\ queue' = Append(queue, [kind |-> "save", epoch |-> monEpoch, copy |-> monCopy, readGen |-> monReadGen])
    /\ mon' = "idle"
    /\ UNCHANGED <<clip, gen, epoch, offs, enabled, history, cleared, requests>>
    /\ UNCHANGED <<started, seen, seenCopy, rebase, monOffs, monFirst, monEpoch, monCopy, monReadGen>>

WriterApplies ==
    /\ queue # <<>>
    /\ LET c == Head(queue) IN
       CASE c.kind = "save" ->
              /\ history' = IF c.epoch = epoch /\ enabled
                            THEN history \cup {[copy |-> c.copy, readGen |-> c.readGen, savedGen |-> gen]}
                            ELSE history
              /\ UNCHANGED <<gen, epoch, offs, enabled, cleared>>
         [] c.kind = "clear" ->
              /\ epoch' = epoch + 1 /\ gen' = gen + 1 /\ history' = {}
              /\ cleared' = cleared \cup {h.copy : h \in history}
              /\ UNCHANGED <<offs, enabled>>
         [] c.kind = "off" ->
              \* UpdateSettings moves the counters only when history was on.
              /\ IF enabled
                 THEN epoch' = epoch + 1 /\ gen' = gen + 1 /\ offs' = offs + 1
                 ELSE UNCHANGED <<epoch, gen, offs>>
              /\ enabled' = FALSE
              /\ UNCHANGED <<history, cleared>>
         [] c.kind = "on" ->
              /\ enabled' = TRUE
              /\ UNCHANGED <<gen, epoch, offs, history, cleared>>
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<clip, requests>>
    /\ UNCHANGED monVars

Done == clip = MaxCopies /\ requests = MaxRequests /\ queue = <<>> /\ mon = "idle" /\ UNCHANGED vars

Next ==
    \/ UserCopies
    \/ UserAsks("clear") \/ UserAsks("off") \/ UserAsks("on")
    \/ MonitorStarts \/ MonitorFinishes \/ MonitorQueuesSave
    \/ WriterApplies
    \/ Done

Spec == Init /\ [][Next]_vars

(* Nothing the monitor read before a Clear or a switch-off is saved after it. *)
NothingStaleIsSaved == \A h \in history : h.readGen = h.savedGen

(* A copy the user cleared never comes back, also after history goes off and on. *)
ClearedNeverReturns == \A h \in history : h.copy \notin cleared

(* What was on the clipboard when TinyDash started is never saved. *)
StartupNeverSaved == \A h \in history : h.copy # 0

TypeOK ==
    /\ mon \in {"idle", "half", "ready"}
    /\ enabled \in BOOLEAN
    /\ epoch = gen
=============================================================================
