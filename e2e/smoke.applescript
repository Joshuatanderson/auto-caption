-- Smoke test: clicks through all 4 pipeline stages in a running Tauri dev build.
-- Usage: osascript e2e/smoke.applescript /abs/path/to/test.mp4
--
-- Prerequisites:
--   1. `bun run tauri dev` is running
--   2. Terminal (or iTerm) has Accessibility permission:
--      System Settings > Privacy & Security > Accessibility
--
-- Tip: You can also test without this script — open DevTools in the Tauri window
--      (Cmd+Option+I) and run:
--      window.__TAURI__.core.invoke('extract_audio', {inputPath: '/path/to/file.mp4'})

on run argv
    if (count of argv) < 1 then
        return "Usage: osascript smoke.applescript /abs/path/to/test.mp4"
    end if
    set videoPath to item 1 of argv

    tell application "System Events"
        tell process "AutoCap"
            set frontmost to true
            delay 0.5

            -- Trigger file picker via Cmd+O (requires keydown listener in the app)
            keystroke "o" using command down
            delay 1.0

            -- Navigate to the file in the sheet
            keystroke "g" using {command down, shift down}
            delay 0.4
            keystroke videoPath
            keystroke return
            delay 0.4
            keystroke return
            delay 1.5

            -- Stage 1: Extract audio
            click button "Extract" of window 1
            delay 8

            -- Stage 2: Transcribe (large-v3-turbo on a 30s clip ≈ 20-45s; generous delay)
            click button "Transcribe" of window 1
            delay 90

            -- Stage 3: Generate .ass
            click button "Generate" of window 1
            delay 5

            -- Stage 4: Burn captions (VideoToolbox is fast)
            click button "Burn" of window 1
            delay 45
        end tell
    end tell

    return "smoke test complete"
end run
