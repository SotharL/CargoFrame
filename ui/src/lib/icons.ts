// The line icons of the app, drawn on a 24×24 grid with a 2 px round stroke (Icon.svelte draws
// them). One place for them, so a tool looks the same on the screenshot, the recording and the
// log viewer, and a new one is one line here.
export const ICONS = {
  // marks and edits
  crop: 'M6 2v16h16M2 6h16v16',
  pen: 'M4 20l1-4L17 4l3 3L8 19z',
  rect: 'M4 5h16v14H4z',
  ellipse: 'M12 5a8 7 0 100 14a8 7 0 100-14',
  arrow: 'M5 19L19 5M10 5h9v9',
  text: 'M5 6V4h14v2M12 4v16M9 20h6',
  read: 'M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4M8 9h8M12 9v7',
  onBox: 'M5 6h14a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2zM8 10h8M12 10v5',
  undo: 'M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 010 11H11',
  redo: 'M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 000 11H13',
  wholePicture: 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5',
  trash: 'M4 7h16M10 7V4h4v3M6 7l1 13h10l1-13M10 11v6M14 11v6',
  eraser: 'M8 20h12M5 15l9-9 5 5-8 8H8z',
  reset: 'M4 4v6h6M5.2 15a8 8 0 101.6-8.4L4 10',
  // the recording
  keys: 'M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10',
  clock: 'M12 3a9 9 0 100 18a9 9 0 100-18M12 7v5l3.5 2.2',
  follow: 'M3 4h11v8H3zM3 7h11M14 18h7M18 15l3 3-3 3',
  camera: 'M4 8h3l2-3h6l2 3h3v11H4zM12 10a3.5 3.5 0 100 7a3.5 3.5 0 100-7',
  // the log viewer
  search: 'M10.5 4a6.5 6.5 0 100 13a6.5 6.5 0 100-13M15.5 15.5L21 21',
  expand: 'M14 4h6v6M20 4l-7 7M10 20H4v-6M4 20l7-7',
  close: 'M6 6l12 12M18 6L6 18',
} as const;

export type IconName = keyof typeof ICONS;
