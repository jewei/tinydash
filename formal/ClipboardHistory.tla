---------------------------- MODULE ClipboardHistory ----------------------------
(***************************************************************************)
(* The clipboard history of TinyDash: the monitor that captures what the   *)
(* user copies, and the engine's single writer that saves it, clears it,   *)
(* and switches history off and on.                                        *)
(*                                                                         *)
(* Code (internal/engine):                                                 *)
(*   MonitorReadsEpoch, MonitorReadsClipboard, MonitorQueuesSave           *)
(*                        = Monitor.Tick (monitor.go)                      *)
(*   WriterApplies "save" = Engine.SaveClip (clipboard.go)                 *)
(*   WriterApplies "clear"= Engine.ClearClipboard (clipboard.go)           *)
(*   WriterApplies "off"/"on" = Engine.SetClipboardHistory (clipboard.go)  *)
(*   queue                = Engine.changes; the writer is Engine.Run        *)
(*   epoch                = Engine.clipEpoch                               *)
(*                                                                         *)
(* EpochFirst is the order inside Monitor.Tick. TRUE is the code. FALSE is *)
(* the broken order, which ClipboardHistoryBroken.cfg checks must fail.    *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS EpochFirst, MaxCopies, MaxRequests

VARIABLES
    clip,      \* what the OS clipboard holds: the number of the copy
    gen,       \* clears and switch-offs the writer applied: real time, not the epoch
    epoch,     \* Engine.clipEpoch
    enabled,   \* Settings.ClipboardHistoryEnabled in the published snapshot
    history,   \* saved entries: [copy, readGen, savedGen]
    queue,     \* changes waiting for the writer, in order
    requests,  \* clears and switches the user asked for, to bound the model
    seen,      \* Monitor.seen: the change counter at the start of the last tick
    mon,       \* where Monitor.Tick is: "idle", "half", "ready"
    monEpoch, monCopy, monReadGen

vars == <<seen, clip, gen, epoch, enabled, history, queue, requests, mon, monEpoch, monCopy, monReadGen>>

Init ==
    /\ clip = 0 /\ gen = 0 /\ epoch = 0 /\ enabled = TRUE
    /\ history = {} /\ queue = <<>> /\ requests = 0
    /\ seen = 0 /\ mon = "idle" /\ monEpoch = 0 /\ monCopy = 0 /\ monReadGen = 0

UserCopies ==
    /\ clip < MaxCopies
    /\ clip' = clip + 1
    /\ UNCHANGED <<seen, gen, epoch, enabled, history, queue, requests, mon, monEpoch, monCopy, monReadGen>>

UserAsks(kind) ==
    /\ requests < MaxRequests
    /\ queue' = Append(queue, [kind |-> kind])
    /\ requests' = requests + 1
    /\ UNCHANGED <<seen, clip, gen, epoch, enabled, history, mon, monEpoch, monCopy, monReadGen>>

\* The monitor's two reads, in the order EpochFirst gives.
ReadEpoch == monEpoch' = epoch
ReadClipboard == monCopy' = clip /\ monReadGen' = gen

\* Tick reads the change counter first and stops when it has not moved.
MonitorStarts ==
    /\ mon = "idle" /\ clip # seen
    /\ seen' = clip
    /\ IF EpochFirst THEN ReadEpoch /\ UNCHANGED <<monCopy, monReadGen>>
                     ELSE ReadClipboard /\ UNCHANGED monEpoch
    /\ mon' = "half"
    /\ UNCHANGED <<clip, gen, epoch, enabled, history, queue, requests>>

MonitorFinishes ==
    /\ mon = "half"
    /\ IF EpochFirst THEN ReadClipboard /\ UNCHANGED monEpoch
                     ELSE ReadEpoch /\ UNCHANGED <<monCopy, monReadGen>>
    /\ mon' = "ready"
    /\ UNCHANGED <<seen, clip, gen, epoch, enabled, history, queue, requests>>

MonitorQueuesSave ==
    /\ mon = "ready"
    /\ queue' = Append(queue, [kind |-> "save", epoch |-> monEpoch, copy |-> monCopy, readGen |-> monReadGen])
    /\ mon' = "idle"
    /\ UNCHANGED <<seen, clip, gen, epoch, enabled, history, requests, monEpoch, monCopy, monReadGen>>

WriterApplies ==
    /\ queue # <<>>
    /\ LET c == Head(queue) IN
       CASE c.kind = "save" ->
              /\ history' = IF c.epoch = epoch /\ enabled
                            THEN history \cup {[copy |-> c.copy, readGen |-> c.readGen, savedGen |-> gen]}
                            ELSE history
              /\ UNCHANGED <<gen, epoch, enabled>>
         [] c.kind = "clear" ->
              /\ epoch' = epoch + 1 /\ gen' = gen + 1 /\ history' = {}
              /\ UNCHANGED enabled
         [] c.kind = "off" ->
              /\ epoch' = epoch + 1 /\ gen' = gen + 1 /\ enabled' = FALSE
              /\ UNCHANGED history
         [] c.kind = "on" ->
              /\ enabled' = TRUE
              /\ UNCHANGED <<gen, epoch, history>>
    /\ queue' = Tail(queue)
    /\ UNCHANGED <<seen, clip, requests, mon, monEpoch, monCopy, monReadGen>>

Done == clip = MaxCopies /\ requests = MaxRequests /\ queue = <<>> /\ mon = "idle" /\ UNCHANGED vars

Next ==
    \/ UserCopies
    \/ UserAsks("clear") \/ UserAsks("off") \/ UserAsks("on")
    \/ MonitorStarts \/ MonitorFinishes \/ MonitorQueuesSave
    \/ WriterApplies
    \/ Done

Spec == Init /\ [][Next]_vars

(* The rule: nothing the monitor read before a Clear or a switch-off is   *)
(* saved after it. A clear empties the history, so this also says that a *)
(* cleared copy never comes back.                                         *)
NothingStaleIsSaved == \A h \in history : h.readGen = h.savedGen

TypeOK ==
    /\ mon \in {"idle", "half", "ready"}
    /\ enabled \in BOOLEAN
    /\ epoch = gen
=============================================================================
