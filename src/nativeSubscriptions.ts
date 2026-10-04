type Stop = () => void;
type Listener = (
  name: string,
  callback: (event: { payload: unknown }) => void,
) => Promise<Stop>;

/** A window owns pending registrations as well as already attached listeners. */
export function createNativeSubscriptions(listen: Listener) {
  let disposed = false;
  const stops = new Set<Stop>();
  async function own(registration: Promise<Stop>) {
    const stop = await registration;
    if (disposed) stop();
    else stops.add(stop);
    return !disposed;
  }
  function guard<T>(callback: (value: T) => void) {
    return (value: T) => {
      if (!disposed) callback(value);
    };
  }
  return {
    own,
    guard,
    get disposed() {
      return disposed;
    },
    register<T = unknown>(name: string, callback: (payload: T) => void) {
      return own(
        listen(
          name,
          guard((event) => callback(event.payload as T)),
        ),
      );
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const stop of stops) stop();
      stops.clear();
    },
  };
}
