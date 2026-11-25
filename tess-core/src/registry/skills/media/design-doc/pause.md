# Targeted media control on Windows

*Tess · media skill · Draft · Author: Akash · 25 Nov 2026*

## Goal

Let voice commands like 'pause the song' or 'pause Spotify' act on the right app on Windows, instead of blindly sending a global key press. The media skill's handlers (`media/pause.rs`, `play.rs`, `next.rs`, `previous.rs`, `skip.rs`, `back.rs`) share one target-resolution step, so every intent behaves the same way.

## Background

The semantic parser picks the intent (for example `media.pause`) and the biotagger extractor tags target tokens, which become the optional `app` arg. In 'pause the song' the target is `song`, which names a kind of media, not an app. So the handler needs its own way to work out which app the user means. Windows exposes this through System Media Transport Controls (SMTC), the system behind the volume-flyout media widget.

## Procedure

1. **Recognize the target.** Read the `app` arg from the command. Treat media words (`song`, `music`, `track`, `video`) as 'no app named'. Anything else is a candidate app name.
2. **Choose how to send the action**, SMTC or virtual media keys, using the priority order below.

### Priority order

1. **App named in the command.** Lowercase the candidate and look for it as a substring in the `SourceAppUserModelId` of each live SMTC session. A match selects that session.
2. **The focused app.** Get the foreground window (`GetForegroundWindow`), its process (`GetWindowThreadProcessId`) and its exe name (`QueryFullProcessImageNameW`), then match the exe name against the session ids. A match selects that session.
3. **Any session that is playing.** Use every session whose playback status is `Playing`.
4. **Media key fallback.** Only if SMTC returns no sessions at all, send the matching virtual media key with `SendInput`.

SMTC is the mechanism for steps 1 to 3 whether or not the app is focused. Focus is only a way to pick the session, not a reason to switch mechanisms.

## Mechanisms

**SMTC** (`windows` crate, `Media_Control` feature, Windows 10 1809 or later). `GlobalSystemMediaTransportControlsSessionManager` gives the list of sessions and the system's current session. Each session reports its app id, playback status and timeline, and accepts control calls.

Intent mapping:

- `media.pause` → `TryPauseAsync`
- `media.play` → `TryPlayAsync`
- `media.next` → `TrySkipNextAsync`
- `media.previous` → `TrySkipPreviousAsync`
- `media.skip` and `media.back` → read the timeline position, add or subtract the duration (in 100 ns ticks), then call `TryChangePlaybackPositionAsync`

The async calls are blocked on with `.get()`, so each handler wraps them in `spawn_blocking`.

**Virtual media keys** (`VK_MEDIA_PLAY_PAUSE`, `VK_MEDIA_NEXT_TRACK`, `VK_MEDIA_PREV_TRACK`). These go to whatever Windows treats as the current media session, not to the focused window, so they cannot target an app. The play/pause key is a toggle. There is no key for skipping by a duration, so `media.skip` and `media.back` fail cleanly when only the fallback is available.

## Resolution sketch

```
fn resolve(app_arg) -> Target:
    sessions = smtc.sessions()
    if sessions.is_empty(): return MediaKey
    if let Some(name) = candidate(app_arg):
        if let Some(s) = match_session(sessions, name): return Sessions([s])
    if let Some(s) = match_session(sessions, focused_exe()): return Sessions([s])
    playing = sessions.filter(Playing)
    if !playing.is_empty(): return Sessions(playing)
    return MediaKey   // or fail, see open questions
```

## Known limitations

- Apps that do not integrate with SMTC are invisible to it.
- Browsers do not reliably expose one session per tab, so 'pause the YouTube tab' cannot be targeted precisely.
- App ids can differ from exe names (for example Store versus desktop builds), so the substring match has to be checked against the real ids on this machine.
- The media key fallback cannot tell pause from play.

## Open questions

- **Resume, next and previous with nothing playing.** Step 3 ('any session that is playing') makes no sense for `media.play`, since nothing is playing. Proposal: use the system's current session (`GetCurrentSession`) for these intents.
- **Several sessions playing.** For `media.pause`, pause all of them, or only the focused or current one? For skip and back, which single session?
- **Fallback for pause and play.** If SMTC finds no sessions, should a toggle key still be sent, or should the command report failure?
- **Aliases.** Spoken names ('chrome', 'spotify', 'youtube') may need a small alias map to session ids.

## Plan

1. Write a small binary that prints every session id and status while Spotify and Chrome are playing, to learn the real ids.
2. Add the `windows` crate with the needed features: `Media_Control`, `Foundation`, `Foundation_Collections`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Threading`.
3. Implement `resolve` in a shared, Windows-only `media` module.
4. Fill in each handler against it, starting with `pause`.

## References

- [Windows.Media.Control namespace](https://learn.microsoft.com/en-us/uwp/api/windows.media.control)
- [GlobalSystemMediaTransportControlsSessionManager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)
- [GlobalSystemMediaTransportControlsSession](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession)