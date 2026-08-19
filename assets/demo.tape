# VHS script for the hyprdesk CLI surface.
# Render with:  vhs assets/demo.tape   ->   assets/cli.gif
#
# Must be run inside a live Hyprland session with the daemon running,
# since every command below talks to the real compositor.

Output assets/cli.gif

Set Shell bash
Set FontSize 20
Set Width 1000
Set Height 560
Set Padding 24
Set Theme "Tokyo Night"
Set TypingSpeed 60ms

Hide
Type "clear" Enter
Show

Sleep 500ms

Type "# A desk spans every monitor. Where are we?"
Enter
Sleep 300ms
Type "hyprdesk status"
Enter
Sleep 1500ms

Type "# Switch every monitor at once."
Enter
Sleep 300ms
Type "hyprdesk vdesk 3"
Enter
Sleep 1500ms

Type "# Step through desks, wrapping at 10."
Enter
Sleep 300ms
Type "hyprdesk nextdesk --cycle"
Enter
Sleep 1200ms

Type "# Jump back to where you came from."
Enter
Sleep 300ms
Type "hyprdesk lastdesk"
Enter
Sleep 1200ms

Type "# Machine readable, for scripts and bars."
Enter
Sleep 300ms
Type "hyprdesk status --json"
Enter
Sleep 2500ms
