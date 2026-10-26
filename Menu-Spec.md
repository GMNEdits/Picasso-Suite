# Picasso Suite - Perfect Match Menu Spec
# DaVinci Resolve 19 Real Menu Structure - 1:1 Mapping

> Source: DaVinci Resolve 19 Reference Manual + UI tree analysis. Clean-room functional description of public menu structure.

## Top Menu Bar (from screenshot) - Exact Match

Your current bar: `Picasso Suite | File | Edit | Trim | Timeline | Clip | Mark | View | Playback | Effects | Color | Audio | Workspace | Help`
This IS exactly Resolve's bar. Keep it.

---

## 1. File Menu - Complete Real Structure

```
File
├── New Project Library...
├── Open Project Library...
├── Show Current Project Library in File Explorer / Finder
├── ---
├── Project Manager (Shift+1)
├── New Project (Ctrl+N / Cmd+N)
├── Open Project...
├── Import Project...
├── Export Project...
├── Restore Project Archive...
├── ---
├── Save Project (Ctrl+S)
├── Save Project As... (Ctrl+Shift+S)
├── Save Copy As...
├── Revert to Last Saved Version
├── ---
├── Import
│   ├── File > Import Media... (Ctrl+I)
│   ├── Folder... (as Bin)
│   ├── Timeline > Import AAF, EDL, XML... (Shift+Cmd+I)
│   ├── Timeline > Import Pre-conformed EDL...
│   ├── Subtitle...
│   ├── Metadata to Media Pool...
│   └── Bin... (Import .drb)
├── Export
│   ├── AAF, XML... (Export Timeline)
│   ├── Subtitle...
│   ├── Bin...
│   └── Metadata from Media Pool...
├── Media Management... (Ctrl+M)
├── ---
├── Project Settings... (Shift+9)
├── System Preferences... (Ctrl+,)
├── ---
├── Quick Export... (Ctrl+Shift+E)
├── ---
└── Quit / Exit (Ctrl+Q)
```

## 2. Edit Menu

```
Edit
├── Undo (Ctrl+Z)
├── Redo (Ctrl+Shift+Z / Ctrl+Y)
├── ---
├── Cut (Ctrl+X)
├── Copy (Ctrl+C)
├── Paste (Ctrl+V)
├── Paste Insert (Ctrl+Shift+V)
├── Paste Mix to Current Track
├── Duplicate (Alt+C) / Duplicate Clip
├── Delete Selected (Backspace / Delete)
├── Delete Selected with Ripple (Shift+Backspace)
├── ---
├── Select All (Ctrl+A)
├── Deselect All (Ctrl+Shift+A)
├── Select Forward / Select Clips Forward
├── Find... (Ctrl+F)
├── ---
├── Project Manager > (recent projects)
└── Preferences > Keyboard Customization (Ctrl+Alt+K)
```

## 3. Trim Menu

```
Trim
├── Select Nearest Edit Point (V)
├── Trim Mode (T)
├── Dynamic Trim Mode (W)
├── Ripple Start (Ctrl+[)
├── Ripple End (Ctrl+])
├── Ripple Trim Previous / Next
├── Trim Start / End
├── Extend Edit (E)
├── Slip Clip (S) / Slip Mode
├── Slide Clip
├── ---
├── Loop Trim
├── Asymmetric Trimming
└── Trim Editor (show)
```

## 4. Timeline Menu

```
Timeline
├── New Timeline... (Ctrl+N)
├── New Timeline Using Selected Clips
├── Create New Compound Clip... (Alt+G)
├── Decompose in Place > (Decompose Compound / Nested)
├── ---
├── Create Timeline Using Bin...
├── Import Timeline...
├── Export Timeline...
├── ---
├── Set Timeline Resolution / Frame Rate (in Project Settings link)
├── Timeline Settings >
│   ├── Snap (N)
│   ├── Linked Selection (Ctrl+Shift+L)
│   ├── Position Lock
│   └── Selection Follows Playhead
├── ---
├── Add Marker (M) / Add Marker with dialog (Alt+M)
├── Delete All Markers
├── ---
├── Create Subtitle Track
├── Create Subtitle Region
├── Add Transition to All Edits (Ctrl+T default = Cross Dissolve)
├── ---
├── Switch Timeline (Ctrl+, / .)
├── Open Timeline in New Window
└── Delete Timeline
```

## 5. Clip Menu

```
Clip
├── Enable Clip (D)
├── Lock Clip
├── ---
├── Add to Media Pool as Stills / PowerGrade / etc
├── Change Clip Speed... (Ctrl+R)
├── Change Clip Duration...
├── Retime Controls (Ctrl+Shift+R)
│   ├── Retime Clip
│   ├── Freeze Frame
│   ├── Speed Ramp
│   └── Reverse Clip
├── ---
├── Create New Compound Clip
├── Decompose in Place
├── ---
├── Link Clips (Ctrl+L)
├── Unlink Audio and Video
├── ---
├── Enable / Disable Audio Scrubbing
├── Auto Align Clips > Based on Timecode / Waveform
├── ---
├── Normalize Audio
├── Render in Place > (Render Fusion Effects, etc)
├── New Fusion Clip
├── New Compound Clip
└── Clip Attributes... (Alt+G in Media Pool)
```

## 6. Mark Menu

```
Mark
├── Mark In (I)
├── Mark Out (O)
├── Mark Clip (X)
├── Mark Timeline In/Out
├── ---
├── Clear In (Alt+I)
├── Clear Out (Alt+O)
├── Clear In and Out (Alt+X)
├── ---
├── Add Marker (M)
├── Add Marker and Show Dialog (Alt+M)
├── Modify Marker...
├── Delete Marker at Current Position
├── Delete All Markers in Timeline / Clip
├── ---
├── Go to Previous Marker (Shift+Up)
├── Go to Next Marker (Shift+Down)
├── ---
└── Set Clip Color > (8 colors + default)
```

## 7. View Menu

```
View
├── Show Current Frame on Playback
├── Safe Area Overlay
├── Show Grid / Guides
├── ---
├── Show Toolbar
├── Show Full Screen Viewer (P / Cmd+F)
├── Show Cinema Viewer (Ctrl+F)
├── ---
├── Viewer Overlays >
│   ├── Transform / Crop / Dynamic Zoom
│   ├── Safe Area / Center Crosshair
│   ├── Checkerboard Background (Alpha)
│   └── Show All Overlays
├── ---
├── Timecode Window
├── Show Scopes (Shift+H on Color)
├── ---
├── Show Audio Meters
├── Show Audio Mixer
└── Show Inspector (top right)
```

## 8. Playback Menu

```
Playback
├── Play Forward (Space / L)
├── Play Reverse (J)
├── Stop (K)
├── ---
├── Play Around Current Frame ( / )
├── Play In to Out
├── Play Around In / Around Out
├── Loop Playback (Ctrl+W)
├── ---
├── Go to First Frame (Home)
├── Go to Last Frame (End)
├── Go to In (Q) / Go to Out (W)
├── Previous Frame (Left Arrow)
├── Next Frame (Right Arrow)
├── Previous Edit Point (Up)
├── Next Edit Point (Down)
├── ---
├── Timeline Playback > Half / Quarter Res
├── Fusion Memory Cache
├── Timeline Proxy Mode > Half / Quarter / Eighth
└── Delete Render Cache
```

## 9. Effects Menu (New in Resolve 19)

```
Effects
├── Video Transitions > (Cross Dissolve, Dip to Color, etc)
├── Audio Transitions > (Cross Fade 0dB, etc)
├── OpenFX > (Blur, Color, Distort, etc)
├── Fairlight FX > (EQ, Dynamics, etc)
├── Resolve FX > (Color, Blur, Light, Stylize, etc)
├── Fusion Effects >
├── Generators > (Solid Color, etc)
├── Titles > (Text, Text+, etc)
├── ---
├── Effects Library Search
└── Add Tool (Fusion)
```

## 10. Color Menu

```
Color
├── Reset All Grades and Nodes (Shift+Home)
├── Reset Current Node Grade
├── Enable/Disable Current Node (Ctrl+D)
├── Grab Still (Ctrl+Alt+G)
├── Play Still (with wipe)
├── ---
├── Nodes >
│   ├── Add Serial Node (Alt+S / S)
│   ├── Add Serial Before
│   ├── Add Parallel Node (Alt+P)
│   ├── Add Layer Mixer (Alt+L)
│   ├── Add Splitter / Combiner
│   ├── Add Corrector / Outside / etc
│   └── Label Node
├── ---
├── Timeline > Previous / Next Node Graph
├── ---
├── LUT > Apply LUT from list
├── Gallery > (PowerGrades, Stills)
├── ---
├── Scopes On/Off
├── Highlight > Blue (Qualifier highlight)
└── Lightbox
```

## 11. Audio Menu (Fairlight)

```
Audio
├── ADR > (Automated Dialogue Replacement)
├── ---
├── Bounce Mix to Track
├── Bounce Selected Tracks
├── ---
├── Cache Audio Effects
├── ---
├── Fairlight Link > Link Group / Decompose
├── Loop Jog
└── Show Audio Timeline / Mixer
```

## 12. Workspace Menu

```
Workspace
├── Full Screen Viewer (P)
├── Show Panel in Workspace >
│   ├── Media Pool
│   ├── Effects Library / Effects
│   ├── Inspector
│   ├── Mixer
│   ├── Metadata
│   ├── Edit Index
│   ├── Sound Library
│   └── etc
├── Layout > Reset UI Layout / Save Layout / Load Layout
├── Switch to Page >
│   ├── Media (Shift+2)
│   ├── Cut (Shift+3)
│   ├── Edit (Shift+4)
│   ├── Fusion (Shift+5)
│   ├── Color (Shift+6)
│   ├── Fairlight (Shift+7)
│   └── Deliver (Shift+8)
├── Dual Screen On/Off
└── Video Clean Feed
```

## 13. Help Menu

```
Help
├── DaVinci Resolve Reference Manual (Shift+? / F1)
├── DaVinci Resolve Training
├── Keyboard Customization (Alt+Cmd+K)
├── ---
├── Check for Updates...
├── Welcome Dialog
├── ---
└── About DaVinci Resolve
```

## Bottom Page Navigation (from screenshot - exact)

```
Bottom bar:
[ Media ] [ Cut ] [ Edit ] [ Fusion ] [ Color ] [ Fairlight ] [ Deliver ]
Icons: 
Media = two reels, Cut = razor, Edit = timeline, Fusion = nodes, Color = color wheel, Fairlight = fader, Deliver = rocket

Your screenshot shows:
- Master (Bin)
- No clips in media pool - placeholder text: "Add clips with Import, or drop files here"
- Viewer centered: "New Project" + timecode 00:00:00:00
- Timeline ruler with TCG 00:00:00:00
- Transport: |< << < [] > >> >| loop etc
```

## Implementation Priority for Perfect Match

Phase 1: Implement File > Import Media, Project Settings, Save
Phase 2: Edit > Undo/Redo, Copy/Paste, Select All
Phase 3: Timeline > New Timeline, Mark > Mark In/Out, Clip > Enable/Disable
Phase 4: Playback > JKL, Go to In/Out
Phase 5: Workspace > Switch to Page (even if pages are placeholders)

This is the 1:1 skeleton Resolve users expect muscle-memory wise.
