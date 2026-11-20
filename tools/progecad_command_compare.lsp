;;; Behavioral comparison for milestone 02.14 in ProgeCAD (docs/testing/
;;; stretch-arcs-windows.md, break-closed-polylines-windows.md,
;;; array-rectangular-windows.md, commands-02-14-windows.md):
;;; synthetic entities made here, stretched or broken by ProgeCAD's own
;;; commands at fixed points, every field written to a log, each case saved
;;; before and after as DWG 2018 and DXF 2018. Load inside ProgeCAD on a
;;; blank drawing from its own template; not a product API.
;;;
;;;   (load "tools/progecad_command_compare.lsp")
;;;   (arq:compare-02-14 "C:/path/to/saved" "C:/path/to/cases.log")
;;;   (arq:compare-array "C:/path/to/array" "C:/path/to/array.log")
;;;   (arq:compare-commands "C:/path/to/cmd" "C:/path/to/commands.log")
;;;
;;; arq:compare-array runs rectangular -ARRAY with ARRAYASSOCIATIVITY 0
;;; (independent copies, as ours) and saves each result as pc-<case>.dwg;
;;; tools/progecad_array_ours.py writes ours-<case>.dwg/.dxf for the same
;;; cases. Reopened, both are described with arq:dump-full.
;;;
;;; Each saved file is then reopened by ProgeCAD from its command line and
;;; described with (arq:dump (vla-get-ActiveDocument (vlax-get-acad-object))
;;; "reopened" log); opening with vla-Open from a script hangs ProgeCAD.
(vl-load-com)

(defun arq:num (x) (rtos x 2 10))
(defun arq:deg (a) (arq:num (* 180.0 (/ a pi))))

(defun arq:lst (v)
  (if (= (type v) 'VARIANT) (setq v (vlax-variant-value v)))
  (if (= (type v) 'SAFEARRAY) (setq v (vlax-safearray->list v)))
  v)

(defun arq:pt (v / l)
  (setq l (arq:lst v))
  (strcat "(" (arq:num (car l)) "," (arq:num (cadr l))
          (if (caddr l) (strcat "," (arq:num (caddr l))) "") ")"))

(defun arq:try (f args / r)
  (setq r (vl-catch-all-apply f args))
  (if (vl-catch-all-error-p r) (strcat "ERR:" (vl-catch-all-error-message r)) r))

(defun arq:polyline (obj / c n i s)
  (setq c (arq:lst (vla-get-Coordinates obj)) n (/ (length c) 2) i 0 s "")
  (while (< i n)
    (setq s (strcat s (if (> i 0) " ; " "")
                    "(" (arq:num (nth (* 2 i) c)) "," (arq:num (nth (1+ (* 2 i)) c)) ")"
                    " bulge=" (arq:num (vla-GetBulge obj i))))
    (vl-catch-all-apply
      '(lambda (/ sw ew)
         (vla-GetWidth obj i 'sw 'ew)
         (setq s (strcat s " w=" (arq:num sw) "/" (arq:num ew)))))
    (setq i (1+ i)))
  (strcat "LWPOLYLINE h=" (vla-get-Handle obj)
          " closed=" (if (= (vla-get-Closed obj) :vlax-true) "yes" "no")
          " elevation=" (arq:num (vla-get-Elevation obj))
          " constwidth=" (arq:try 'arq:cw (list obj))
          " normal=" (arq:pt (vla-get-Normal obj))
          " vertices=" (itoa n) " [" s "]"))

(defun arq:cw (obj) (arq:num (vla-get-ConstantWidth obj)))

(defun arq:describe (obj / k)
  (setq k (vla-get-ObjectName obj))
  (cond
    ((= k "AcDbArc")
     (strcat "ARC h=" (vla-get-Handle obj)
             " center=" (arq:pt (vla-get-Center obj))
             " radius=" (arq:num (vla-get-Radius obj))
             " start=" (arq:deg (vla-get-StartAngle obj))
             " end=" (arq:deg (vla-get-EndAngle obj))
             " included=" (arq:try 'arq:total (list obj))
             " normal=" (arq:pt (vla-get-Normal obj))
             " startpt=" (arq:pt (vla-get-StartPoint obj))
             " endpt=" (arq:pt (vla-get-EndPoint obj))))
    ((= k "AcDbPolyline") (arq:polyline obj))
    ((= k "AcDbLine")
     (strcat "LINE h=" (vla-get-Handle obj) " " (arq:pt (vla-get-StartPoint obj))
             " -> " (arq:pt (vla-get-EndPoint obj))))
    ((= k "AcDbCircle")
     (strcat "CIRCLE h=" (vla-get-Handle obj) " center=" (arq:pt (vla-get-Center obj))
             " radius=" (arq:num (vla-get-Radius obj))))
    ((= k "AcDbText")
     (strcat "TEXT h=" (vla-get-Handle obj) " at=" (arq:pt (vla-get-InsertionPoint obj))
             " value=" (vla-get-TextString obj) " height=" (arq:num (vla-get-Height obj))
             " style=" (vla-get-StyleName obj) " width=" (arq:num (vla-get-ScaleFactor obj))
             " oblique=" (arq:deg (vla-get-ObliqueAngle obj))
             " rotation=" (arq:deg (vla-get-Rotation obj))))
    ((= k "AcDbEllipse")
     (strcat "ELLIPSE h=" (vla-get-Handle obj) " center=" (arq:pt (vla-get-Center obj))
             " major=" (arq:pt (vla-get-MajorAxis obj)) " ratio=" (arq:num (vla-get-RadiusRatio obj))
             " start=" (arq:num (vla-get-StartParameter obj))
             " end=" (arq:num (vla-get-EndParameter obj))))
    ((= k "AcDbMText")
     (strcat "MTEXT h=" (vla-get-Handle obj) " at=" (arq:pt (vla-get-InsertionPoint obj))
             " value=" (vla-get-TextString obj) " height=" (arq:num (vla-get-Height obj))
             " style=" (vla-get-StyleName obj)))
    (t (strcat k " h=" (vla-get-Handle obj)))))

(defun arq:dump (doc label log / ms n d)
  (setq ms (vla-get-ModelSpace doc) n 0)
  (arq:mark log (strcat "  " label ":"))
  (vlax-for obj ms
    (setq n (1+ n))
    (setq d (vl-catch-all-apply 'arq:describe (list obj)))
    (arq:mark log (strcat "    " (if (vl-catch-all-error-p d)
                                     (strcat (vla-get-ObjectName obj) " describe-error: " (vl-catch-all-error-message d))
                                     d))))
  (arq:mark log (strcat "    (" (itoa n) " entities)")))

(defun arq:mark (path text / file)
  (setq file (open path "a"))
  (write-line text file)
  (close file)
  text)

(defun arq:v (p) (vlax-3d-point (car p) (cadr p) (if (caddr p) (caddr p) 0.0)))

(defun arq:clear (doc / ms l)
  (setq ms (vla-get-ModelSpace doc) l '())
  (vlax-for obj ms (setq l (cons obj l)))
  (foreach obj l (vla-Delete obj)))

;;; An ARC by center, radius and angles in degrees.
(defun arq:arc (doc c r a0 a1)
  (vla-AddArc (vla-get-ModelSpace doc) (arq:v c) r (* pi (/ a0 180.0)) (* pi (/ a1 180.0))))

;;; A closed LWPOLYLINE through 2D points, with bulges and a constant width.
(defun arq:closed (doc pts bulges width / arr pl i)
  (setq arr (vlax-make-safearray vlax-vbDouble (cons 0 (1- (* 2 (length pts))))))
  (vlax-safearray-fill arr (apply 'append (mapcar '(lambda (p) (list (car p) (cadr p))) pts)))
  (setq pl (vla-AddLightWeightPolyline (vla-get-ModelSpace doc) arr))
  (vla-put-Closed pl :vlax-true)
  (setq i 0)
  (foreach b bulges (vla-SetBulge pl i b) (setq i (1+ i)))
  (if width (vla-put-ConstantWidth pl width))
  pl)

(defun arq:save (doc dir name log / r dwg dxf)
  (setq dwg (strcat dir "/" name ".dwg") dxf (strcat dir "/" name ".dxf"))
  (setq r (vl-catch-all-apply 'vla-SaveAs (list doc dwg 64)))
  (arq:mark log (strcat "    saved " name ".dwg: " (if (vl-catch-all-error-p r) (vl-catch-all-error-message r) "ok")))
  (setq r (vl-catch-all-apply 'vla-SaveAs (list doc dxf 65)))
  (arq:mark log (strcat "    saved " name ".dxf: " (if (vl-catch-all-error-p r) (vl-catch-all-error-message r) "ok"))))

(defun arq:prepare ()
  (setvar "CMDECHO" 0)
  (setq arq:old (list (getvar "OSMODE") (getvar "ORTHOMODE") (getvar "SNAPMODE")))
  (setvar "OSMODE" 0)
  (setvar "ORTHOMODE" 0)
  (setvar "SNAPMODE" 0)
  (command "_.UCS" "_W"))

(defun arq:restore ()
  (setvar "OSMODE" (nth 0 arq:old))
  (setvar "ORTHOMODE" (nth 1 arq:old))
  (setvar "SNAPMODE" (nth 2 arq:old)))

(defun arq:view () (command "_.ZOOM" "_W" '(-14 -14) '(16 16)))

;;; One case: build, record, save; act; record, save.
(defun arq:case (name make act dir log / doc r k)
  (setq doc (vla-get-ActiveDocument (vlax-get-acad-object)))
  (arq:mark log (strcat "case " name))
  (arq:clear doc)
  (setq arq:e (eval make))
  (arq:view)
  (arq:dump doc "before" log)
  (if dir (arq:save doc dir (strcat name "-before") log))
  (setq r (vl-catch-all-apply 'eval (list act)))
  (if (vl-catch-all-error-p r) (arq:mark log (strcat "  command error: " (vl-catch-all-error-message r))))
  (setq k 0)
  (while (and (= 1 (logand 1 (getvar "CMDACTIVE"))) (< k 5)) (command "") (setq k (1+ k)))
  (if (= 1 (logand 1 (getvar "CMDACTIVE"))) (arq:mark log "  command still active after 5 Enters"))
  (arq:dump doc "after" log)
  (if dir (arq:save doc dir (strcat name "-after") log)))

(defun arq:total (obj / d)
  (setq d (- (vla-get-EndAngle obj) (vla-get-StartAngle obj)))
  (if (< d 0) (setq d (+ d (* 2 pi))))
  (arq:deg d))

;;; Every case of 2026-11-15, in order. Object snaps, ortho and snap are
;;; switched off for the run and restored afterwards; the UCS is set to World.
(defun arq:compare-02-14 (dir log / sq)
  (arq:mark log (strcat "ProgeCAD " (getvar "ACADVER") " insunits=" (itoa (getvar "INSUNITS"))
                        " measurement=" (itoa (getvar "MEASUREMENT"))))
  (arq:prepare)
  (setq sq '((0 0) (10 0) (10 10) (0 10)))
  ;; STRETCH: a standalone ARC with one endpoint in the crossing window.
  (arq:case "A1-arc90-end" '(arq:arc doc '(0 0 0) 5.0 0.0 90.0) '(command "_.STRETCH" "_C" '(-1 4) '(1 6) "" '(0 0) '(3 2)) dir log)
  (arq:case "A2-arc90-start" '(arq:arc doc '(0 0 0) 5.0 0.0 90.0) '(command "_.STRETCH" "_C" '(4 -1) '(6 1) "" '(0 0) '(3 2)) dir log)
  (arq:case "A3-arc180-end" '(arq:arc doc '(0 0 0) 5.0 0.0 180.0) '(command "_.STRETCH" "_C" '(-6 -1) '(-4 1) "" '(0 0) '(3 2)) dir log)
  (arq:case "A3b-arc180-start" '(arq:arc doc '(0 0 0) 5.0 0.0 180.0) '(command "_.STRETCH" "_C" '(4 -1) '(6 1) "" '(0 0) '(3 2)) dir log)
  (arq:case "A4-arc270-end" '(arq:arc doc '(0 0 0) 5.0 0.0 270.0) '(command "_.STRETCH" "_C" '(-1 -6) '(1 -4) "" '(0 0) '(3 2)) dir log)
  (arq:case "A4b-arc270-start" '(arq:arc doc '(0 0 0) 5.0 0.0 270.0) '(command "_.STRETCH" "_C" '(4 -1) '(6 1) "" '(0 0) '(3 2)) dir log)
  (arq:case "A5-arc90-elevation2-end" '(arq:arc doc '(0 0 2) 5.0 0.0 90.0) '(command "_.STRETCH" "_C" '(-1 4) '(1 6) "" '(0 0) '(3 2)) dir log)
  (arq:case "A6-control-whole-arc" '(arq:arc doc '(0 0 0) 5.0 0.0 90.0) '(command "_.STRETCH" (vlax-vla-object->ename arq:e) "" '(0 0) '(3 2)) dir log)
  (arq:case "A7-control-crossing-both-ends" '(arq:arc doc '(0 0 0) 5.0 0.0 90.0) '(command "_.STRETCH" "_C" '(-1 -1) '(6 6) "" '(0 0) '(3 2)) dir log)
  (arq:case "A8-degenerate-end-onto-start" '(arq:arc doc '(0 0 0) 5.0 0.0 90.0) '(command "_.STRETCH" "_C" '(-1 4) '(1 6) "" '(0 0) '(5 -5)) dir log)
  ;; BREAK: a closed LWPOLYLINE, first point given with the F option.
  (arq:case "B1-ccw-square-10_5-to-5_10" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 5) '(5 10)) dir log)
  (arq:case "B2-ccw-square-5_10-to-10_5" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(5 10) '(10 5)) dir log)
  (arq:case "B3-ccw-square-across-start-0_5-to-5_0" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(0 5) '(5 0)) dir log)
  (arq:case "B4-cw-square-10_5-to-5_10" '(arq:closed doc '((0 0) (0 10) (10 10) (10 0)) nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 5) '(5 10)) dir log)
  (arq:case "B5-ccw-square-single-point-10_5" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 5) "@") dir log)
  (arq:case "B6-bulged-top-10_2-to-5_10" '(arq:closed doc '((0 0) (10 0) (10 5) (0 5)) '(0.0 0.0 1.0 0.0) nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 2) '(5 10)) dir log)
  (arq:case "B7-width-square-10_5-to-5_10" '(arq:closed doc sq nil 0.5) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 5) '(5 10)) dir log)
  (arq:case "B8-same-segment-2_0-to-8_0" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(2 0) '(8 0)) dir log)
  (arq:case "B9-same-segment-8_0-to-2_0" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(8 0) '(2 0)) dir log)
  (arq:case "B10-second-at-vertex-10_5-to-10_10" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 5) '(10 10)) dir log)
  (arq:case "B11-both-at-vertices-10_0-to-0_10" '(arq:closed doc sq nil nil) '(command "_.BREAK" (vlax-vla-object->ename arq:e) "_F" '(10 0) '(0 10)) dir log)
  (arq:restore)
  (arq:mark log "run done"))

(defun arq:full (obj / d)
  (setq d (arq:describe obj))
  (strcat d " layer=" (vla-get-Layer obj) " color=" (itoa (vla-get-Color obj)) (arq:extra obj)))

;;; The properties MATCHPROP gives, those the object has (added 2026-11-17).
(defun arq:extra (obj / s v)
  (setq s "")
  (foreach pair (list (cons " ltype=" 'vla-get-Linetype) (cons " ltscale=" 'vla-get-LinetypeScale)
                      (cons " lweight=" 'vla-get-Lineweight) (cons " thickness=" 'vla-get-Thickness)
                      (cons " transparency=" 'vla-get-EntityTransparency))
    (setq v (vl-catch-all-apply (cdr pair) (list obj)))
    (if (not (vl-catch-all-error-p v))
      (setq s (strcat s (car pair)
                      (cond ((= (type v) 'STR) v) ((= (type v) 'INT) (itoa v)) (T (arq:num v)))))))
  s)

(defun arq:dump-full (doc label log / ms n d)
  (setq ms (vla-get-ModelSpace doc) n 0)
  (arq:mark log (strcat "  " label ":"))
  (vlax-for obj ms
    (setq n (1+ n))
    (setq d (vl-catch-all-apply 'arq:full (list obj)))
    (arq:mark log (strcat "    " (if (vl-catch-all-error-p d)
                                     (strcat (vla-get-ObjectName obj) " describe-error: " (vl-catch-all-error-message d))
                                     d))))
  (arq:mark log (strcat "    (" (itoa n) " entities)")))

(defun arq:count (doc / n) (setq n 0) (vlax-for obj (vla-get-ModelSpace doc) (setq n (1+ n))) n)

(defun arq:sources (doc kind / ms l c a p lay)
  (setq ms (vla-get-ModelSpace doc))
  (cond
    ((= kind 'line) (list (vla-AddLine ms (arq:v '(0 0 0)) (arq:v '(1 0.5 0)))))
    ((= kind 'many)
     (if (not (tblsearch "LAYER" "ARRAY-TEST")) (vla-Add (vla-get-Layers doc) "ARRAY-TEST"))
     (setq l (vla-AddLine ms (arq:v '(0 0 0)) (arq:v '(1 0.5 0))))
     (vla-put-Layer l "ARRAY-TEST")
     (vla-put-Color l 1)
     (setq c (vla-AddCircle ms (arq:v '(5 5 0)) 1.0))
     (setq a (arq:arc doc '(8 1 0) 1.0 0.0 90.0))
     (setq p (arq:closed doc '((10 0) (12 0) (12 2) (10 2)) nil 0.25))
     (list l c a p))
    ((= kind 'elevated)
     (setq p (arq:closed doc '((0 0) (1 0) (1 1) (0 1)) nil nil))
     (vla-put-Elevation p 2.0)
     (list p))))

(defun arq:ss (objs / ss)
  (setq ss (ssadd))
  (foreach o objs (ssadd (vlax-vla-object->ename o) ss))
  ss)

;;; One -ARRAY case: sources, the command's own arguments, saved and described.
(defun arq:array-case (name kind args angle dir log count-only / doc r k)
  (setq doc (vla-get-ActiveDocument (vlax-get-acad-object)))
  (arq:mark log (strcat "case " name))
  (arq:clear doc)
  (setvar "SNAPANG" (* pi (/ angle 180.0)))
  (setq arq:objs (arq:sources doc kind))
  (command "_.ZOOM" "_W" '(-200 -200) '(200 200))
  (setq r (vl-catch-all-apply 'eval (list (append (list 'command "_.-ARRAY" (arq:ss arq:objs) "" "_R") args))))
  (if (vl-catch-all-error-p r) (arq:mark log (strcat "  command error: " (vl-catch-all-error-message r))))
  (setq k 0)
  (while (and (= 1 (logand 1 (getvar "CMDACTIVE"))) (< k 5)) (command "") (setq k (1+ k)))
  (if (> k 0) (arq:mark log (strcat "  command needed " (itoa k) " Enter(s) to end")))
  (setvar "SNAPANG" 0.0)
  (if count-only
    (arq:mark log (strcat "  after: " (itoa (arq:count doc)) " entities"))
    (progn
      (arq:dump-full doc "after" log)
      (if dir (vl-catch-all-apply 'vla-SaveAs (list doc (strcat dir "/pc-" name ".dwg") 64))))))

(defun arq:compare-array (dir log / assoc0)
  (arq:mark log (strcat "ProgeCAD " (getvar "ACADVER") " ARRAYASSOCIATIVITY=" (vl-princ-to-string (getvar "ARRAYASSOCIATIVITY"))))
  (arq:prepare)
  (setq assoc0 (getvar "ARRAYASSOCIATIVITY"))
  (setvar "ARRAYASSOCIATIVITY" 0)
  (arq:array-case "C1-basic-3x4" 'line '(3 4 2 3) 0.0 dir log nil)
  (arq:array-case "C2-negative-3x4" 'line '(3 4 -2 -3) 0.0 dir log nil)
  (arq:array-case "C3-snapang30-3x4" 'line '(3 4 2 3) 30.0 dir log nil)
  (arq:array-case "C4-many-sources-2x3" 'many '(2 3 4 15) 0.0 dir log nil)
  (arq:array-case "C5-one-row-1x5" 'line '(1 5 3) 0.0 dir log nil)
  (arq:array-case "C6-one-column-5x1" 'line '(5 1 2) 0.0 dir log nil)
  (arq:array-case "C7-elevated-2x2" 'elevated '(2 2 3 3) 0.0 dir log nil)
  (arq:array-case "C8-zero-spacing-3x3" 'line '(3 3 0 0) 0.0 dir log nil)
  (arq:array-case "C9-over-limit-317x316" 'line '(317 316 2 3) 0.0 nil log t)
  (setvar "ARRAYASSOCIATIVITY" assoc0)
  (arq:restore)
  (arq:mark log (strcat "ARRAYASSOCIATIVITY restored=" (vl-princ-to-string (getvar "ARRAYASSOCIATIVITY"))))
  (arq:mark log "run done"))

;;; The 2D commands of milestone 02.14 in ProgeCAD (docs/testing/
;;; commands-02-14-windows.md): each case's synthetic sources, ProgeCAD's own
;;; command at fixed points (TRIM in its Quick mode, as ours; EXTEND with the
;;; boundary given, as ours finds it),
;;; the result saved as pc-<case>.dwg and described with layer and color.
;;; tools/progecad_02_14_ours.py runs the same cases through our core.

(defun arq:ln (doc a b) (vla-AddLine (vla-get-ModelSpace doc) (arq:v a) (arq:v b)))
(defun arq:circ (doc c r) (vla-AddCircle (vla-get-ModelSpace doc) (arq:v c) r))
;;; An ELLIPSE about c, its major semi-axis the vector major, cut to the
;;; parameters p0 to p1 (radians) unless they are nil; returned.
(defun arq:ell (doc c major ratio p0 p1 / e)
  (setq e (vla-AddEllipse (vla-get-ModelSpace doc) (arq:v c) (arq:v major) ratio))
  (if p0 (vla-put-StartParameter e p0))
  (if p1 (vla-put-EndParameter e p1))
  e)
(defun arq:en (o) (vlax-vla-object->ename o))

;;; POLYLINE with a constant width, returned.
(defun arq:wide (pl width) (vla-put-ConstantWidth pl width) pl)

;;; An object's thickness, color 1 (red) or layer set (made when missing), returned.
(defun arq:thick (o th) (vla-put-Thickness o th) o)
(defun arq:red (o) (vla-put-Color o 1) o)
(defun arq:on-layer (o doc name)
  (vl-catch-all-apply 'vla-Add (list (vla-get-Layers doc) name))
  (vla-put-Layer o name) o)

;;; A text, an MTEXT 10 wide, or a text given style ROTULO (made when
;;; missing), width factor 0.8, oblique 15 and rotation 30, returned.
(defun arq:txt (doc s p h) (vla-AddText (vla-get-ModelSpace doc) s (arq:v p) h))
(defun arq:mtxt (doc s p h / m)
  (setq m (vla-AddMText (vla-get-ModelSpace doc) (arq:v p) 10.0 s))
  (vla-put-Height m h) m)
(defun arq:look (o doc)
  (vl-catch-all-apply 'vla-Add (list (vla-get-TextStyles doc) "ROTULO"))
  (vla-put-StyleName o "ROTULO") (vla-put-ScaleFactor o 0.8)
  (vla-put-ObliqueAngle o (* pi (/ 15.0 180.0))) (vla-put-Rotation o (* pi (/ 30.0 180.0))) o)

;;; A polyline with each segment's start and end widths, returned.
(defun arq:widths (pl pairs / i)
  (setq i 0)
  (foreach w pairs (vla-SetWidth pl i (car w) (cadr w)) (setq i (1+ i)))
  pl)

;;; An open LWPOLYLINE through 2D points, with bulges.
(defun arq:open (doc pts bulges / arr pl i)
  (setq arr (vlax-make-safearray vlax-vbDouble (cons 0 (1- (* 2 (length pts))))))
  (vlax-safearray-fill arr (apply 'append (mapcar '(lambda (p) (list (car p) (cadr p))) pts)))
  (setq pl (vla-AddLightWeightPolyline (vla-get-ModelSpace doc) arr))
  (setq i 0)
  (foreach b bulges (vla-SetBulge pl i b) (setq i (1+ i)))
  pl)

;;; One case: make returns the source objects (arq:o), act runs the command.
(defun arq:command-case (name make act dir log / doc r k)
  (setq doc (vla-get-ActiveDocument (vlax-get-acad-object)))
  (arq:mark log (strcat "case " name))
  (arq:clear doc)
  (setq arq:o (eval make))
  (command "_.ZOOM" "_W" '(-20 -20) '(30 30))
  (setq r (vl-catch-all-apply 'eval (list act)))
  (if (vl-catch-all-error-p r) (arq:mark log (strcat "  command error: " (vl-catch-all-error-message r))))
  (setq k 0)
  (while (and (= 1 (logand 1 (getvar "CMDACTIVE"))) (< k 5)) (command "") (setq k (1+ k)))
  (if (> k 0) (arq:mark log (strcat "  needed " (itoa k) " Enter(s) to end")))
  (arq:dump-full doc "after" log)
  (if dir (vl-catch-all-apply 'vla-SaveAs (list doc (strcat dir "/pc-" name ".dwg") 64))))

;;; RECTANG remembers its options from one rectangle to the next, and
;;; several options in one command are not taken from a script: each is set
;;; by a rectangle of its own, far away and erased.
(defun arq:rect-option (option / before e)
  (setq before (entlast))
  (eval (append '(command "_.RECTANG") option '((1000 1000) (1001 1001))))
  (setq e (entlast))
  (if (and e (not (equal e before)))
    (vla-Delete (vlax-ename->vla-object e))))

(defun arq:rectang (fillet chamfer width elevation)
  (if chamfer (arq:rect-option (list "_C" (car chamfer) (cadr chamfer))) (arq:rect-option (list "_F" fillet)))
  (arq:rect-option (list "_W" width))
  (arq:rect-option (list "_E" elevation))
  (arq:rect-option '("_T" 0.0))
  (command "_.RECTANG" '(0 0) '(10 6)))

;;; EXTEND in its standard mode: the boundary given, the object picked by a
;;; point (a picked entity with its point is not taken from a script).
(defun arq:extend (boundary at / mode)
  (setq mode (getvar "TRIMEXTENDMODE"))
  (setvar "TRIMEXTENDMODE" 0)
  (command "_.EXTEND" (arq:en boundary) "" at "")
  (setvar "TRIMEXTENDMODE" mode))

(defun arq:compare-commands (dir log / sq)
  (arq:mark log (strcat "ProgeCAD " (getvar "ACADVER") " TRIMEXTENDMODE="
                        (vl-princ-to-string (getvar "TRIMEXTENDMODE"))))
  (arq:prepare)
  (setq sq '((0 0) (10 0) (10 6) (0 6)))
  ;; OFFSET
  (arq:command-case "O1-offset-line" '(list (arq:ln doc '(0 0) '(10 0))) '(command "_.OFFSET" 2.0 (list (arq:en (car arq:o)) '(5 0)) '(5 5) "") dir log)
  (arq:command-case "O2-offset-circle-out" '(list (arq:circ doc '(0 0) 5.0)) '(command "_.OFFSET" 1.0 (list (arq:en (car arq:o)) '(5 0)) '(10 0) "") dir log)
  (arq:command-case "O3-offset-arc-in" '(list (arq:arc doc '(0 0 0) 5.0 0.0 90.0)) '(command "_.OFFSET" 1.0 (list (arq:en (car arq:o)) '(3.5355339 3.5355339)) '(1 1) "") dir log)
  (arq:command-case "O4-offset-closed-polyline-out" '(list (arq:closed doc sq nil nil)) '(command "_.OFFSET" 1.0 (list (arq:en (car arq:o)) '(5 0)) '(-5 -5) "") dir log)
  (arq:command-case "O5-offset-open-polyline-with-arc" '(list (arq:open doc '((0 0) (10 0) (10 6)) '(0.0 0.41421356237))) '(command "_.OFFSET" 1.0 (list (arq:en (car arq:o)) '(5 0)) '(5 5) "") dir log)
  ;; TRIM, Quick mode, the object picked by a point; EXTEND to the boundary given
  (arq:command-case "T1-trim-line-end" '(list (arq:ln doc '(0 0) '(10 0)) (arq:ln doc '(5 -5) '(5 5))) '(command "_.TRIM" '(8 0) "") dir log)
  (arq:command-case "T2-trim-circle-top" '(list (arq:circ doc '(0 0) 5.0) (arq:ln doc '(-10 0) '(10 0))) '(command "_.TRIM" '(0 5) "") dir log)
  (arq:command-case "T3-trim-line-middle" '(list (arq:ln doc '(0 0) '(10 0)) (arq:ln doc '(3 -5) '(3 5)) (arq:ln doc '(7 -5) '(7 5))) '(command "_.TRIM" '(5 0) "") dir log)
  (arq:command-case "E1-extend-line" '(list (arq:ln doc '(0 0) '(4 0)) (arq:ln doc '(10 -5) '(10 5))) '(arq:extend (cadr arq:o) '(3 0)) dir log)
  (arq:command-case "E2-extend-arc" '(list (arq:arc doc '(0 0 0) 5.0 0.0 45.0) (arq:ln doc '(0 -10) '(0 10))) '(arq:extend (cadr arq:o) '(3.9931 3.0091)) dir log)
  ;; STRETCH, before FILLET and CHAMFER: after a polyline CHAMFER, ProgeCAD's
  ;; next STRETCH selects nothing
  (arq:command-case "S1-stretch-polyline-side" '(list (arq:closed doc sq nil nil)) '(command "_.STRETCH" "_C" '(12 7) '(8 -1) "" '(0 0) '(3 0)) dir log)
  (arq:command-case "S2-stretch-line-end" '(list (arq:ln doc '(0 0) '(10 0))) '(command "_.STRETCH" "_C" '(12 1) '(8 -1) "" '(0 0) '(3 2)) dir log)
  ;; FILLET and CHAMFER
  (arq:command-case "F1-fillet-lines" '(list (arq:ln doc '(0 0) '(8 0)) (arq:ln doc '(10 2) '(10 10))) '(progn (command "_.FILLET" "_R" 2.0) (command "_.FILLET" (list (arq:en (car arq:o)) '(4 0)) (list (arq:en (cadr arq:o)) '(10 8)))) dir log)
  (arq:command-case "F2-fillet-polyline" '(list (arq:closed doc sq nil nil)) '(progn (command "_.FILLET" "_R" 1.0) (command "_.FILLET" "_P" (arq:en (car arq:o)))) dir log)
  (arq:command-case "C1-chamfer-lines" '(list (arq:ln doc '(0 0) '(8 0)) (arq:ln doc '(10 2) '(10 10))) '(progn (command "_.CHAMFER" "_D" 2.0 3.0) (command "_.CHAMFER" (list (arq:en (car arq:o)) '(4 0)) (list (arq:en (cadr arq:o)) '(10 8)))) dir log)
  (arq:command-case "C2-chamfer-polyline" '(list (arq:closed doc sq nil nil)) '(progn (command "_.CHAMFER" "_D" 1.0 1.0) (command "_.CHAMFER" "_P" (arq:en (car arq:o)))) dir log)
  ;; BREAK
  (arq:command-case "B1-break-line" '(list (arq:ln doc '(0 0) '(10 0))) '(command "_.BREAK" (arq:en (car arq:o)) "_F" '(3 0) '(7 0)) dir log)
  (arq:command-case "B2-break-circle" '(list (arq:circ doc '(0 0) 5.0)) '(command "_.BREAK" (arq:en (car arq:o)) "_F" '(5 0) '(0 5)) dir log)
  (arq:command-case "B3-break-arc" '(list (arq:arc doc '(0 0 0) 5.0 0.0 180.0)) '(command "_.BREAK" (arq:en (car arq:o)) "_F" '(4.330127019 2.5) '(2.5 4.330127019)) dir log)
  (arq:command-case "B4-break-open-polyline" '(list (arq:open doc '((0 0) (10 0) (10 10)) nil)) '(command "_.BREAK" (arq:en (car arq:o)) "_F" '(5 0) '(10 5)) dir log)
  ;; JOIN
  (arq:command-case "J1-join-collinear-lines" '(list (arq:ln doc '(0 0) '(4 0)) (arq:ln doc '(6 0) '(10 0))) '(command "_.JOIN" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  (arq:command-case "J2-join-arcs" '(list (arq:arc doc '(0 0 0) 5.0 0.0 90.0) (arq:arc doc '(0 0 0) 5.0 180.0 270.0)) '(command "_.JOIN" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; JOIN on polylines (second delivery): what to confirm against ours,
  ;; see docs/design/join-command.md
  ;; J3: a polyline with a constant width 0.5 and a line: the joined segment's width.
  (arq:command-case "J3-join-polyline-width-line" '(list (arq:wide (arq:open doc '((0 0) (4 0)) '(0)) 0.5) (arq:ln doc '(4 0) '(4 3))) '(command "_.JOIN" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; J4: widths 0.2 and 0.8, the second polyline running the other way.
  (arq:command-case "J4-join-polylines-two-widths" '(list (arq:wide (arq:open doc '((0 0) (4 0)) '(0)) 0.2) (arq:wide (arq:open doc '((8 0) (4 0)) '(0)) 0.8)) '(command "_.JOIN" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; J5: four lines of a rectangle, many at once: a closed polyline?
  (arq:command-case "J5-join-rectangle-at-once" '(list (arq:ln doc '(0 0) '(4 0)) (arq:ln doc '(4 0) '(4 3)) (arq:ln doc '(0 3) '(4 3)) (arq:ln doc '(0 0) '(0 3))) '(command "_.JOIN" (arq:ss arq:o) "") dir log)
  ;; J6: a branch at the source's end: which line joins?
  (arq:command-case "J6-join-polyline-branch" '(list (arq:open doc '((0 0) (1 0)) '(0)) (arq:ln doc '(1 0) '(2 1)) (arq:ln doc '(1 0) '(2 -1))) '(command "_.JOIN" (arq:en (car arq:o)) (arq:en (cadr arq:o)) (arq:en (caddr arq:o)) "") dir log)
  ;; J7: a line and an arc touching, many at once: a polyline with the line's properties.
  (arq:command-case "J7-join-line-arc-at-once" '(list (arq:ln doc '(0 0) '(4 0)) (arq:arc doc '(4 2 0) 2.0 270.0 90.0)) '(command "_.JOIN" (arq:ss arq:o) "") dir log)
  ;; MATCHPROP, every setting on (docs/design/matchprop-command.md)
  ;; M1: a line on A-WALLS, red, gives them to a circle.
  (arq:command-case "M1-matchprop-line-to-circle" '(list (arq:on-layer (arq:red (arq:ln doc '(0 0) '(4 0))) doc "A-WALLS") (arq:circ doc '(8 0) 1.0)) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; M2: a polyline with width 0.5 and thickness 2 gives them to a plain one.
  (arq:command-case "M2-matchprop-polyline-width" '(list (arq:thick (arq:wide (arq:open doc '((0 0) (4 0)) '(0)) 0.5) 2.0) (arq:open doc '((0 2) (4 2)) '(0))) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; M3: a varying width (0.2 then 0.8) is not given: the target keeps 0.25.
  (arq:command-case "M3-matchprop-varying-width" '(list (arq:widths (arq:open doc '((0 0) (4 0) (8 0)) '(0 0)) '((0.2 0.2) (0.8 0.8))) (arq:wide (arq:open doc '((0 2) (4 2)) '(0)) 0.25)) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; MATCHPROP's Text: what ProgeCAD gives besides the style (to confirm).
  ;; M4: a text 2.5 high gives its height to one 1.0 high?
  (arq:command-case "M4-matchprop-text-height" '(list (arq:txt doc "A-101" '(0 0) 2.5) (arq:txt doc "B-202" '(0 5) 1.0)) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; M5: a text gives its height to an MTEXT?
  (arq:command-case "M5-matchprop-text-to-mtext" '(list (arq:txt doc "A-101" '(0 0) 2.5) (arq:mtxt doc "Notes" '(0 5) 1.0)) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; M6 (ProgeCAD only: ours cannot make this source yet): a text in style
  ;; ROTULO, width factor 0.8, oblique 15 degrees, rotated 30 degrees.
  ;; Which of style, width, oblique (and not rotation) does it give?
  (arq:command-case "M6-matchprop-text-look" '(list (arq:look (arq:txt doc "A-101" '(0 0) 2.5) doc) (arq:txt doc "B-202" '(0 5) 1.0)) '(command "_.MATCHPROP" (arq:en (car arq:o)) (arq:en (cadr arq:o)) "") dir log)
  ;; ELLIPSE (docs/design/ellipse-command.md)
  (arq:command-case "EL1-ellipse-axis-distance" 'nil '(command "_.ELLIPSE" '(0 0) '(10 0) 3) dir log)
  (arq:command-case "EL2-ellipse-center" 'nil '(command "_.ELLIPSE" "_C" '(5 0) '(7 0) 4) dir log)
  (arq:command-case "EL3-ellipse-rotation" 'nil '(command "_.ELLIPSE" '(0 0) '(10 0) "_R" 60) dir log)
  ;; EL4: angles from the first axis drawn, which points west from (5,0).
  (arq:command-case "EL4-ellipse-arc" 'nil '(command "_.ELLIPSE" "_A" '(0 0) '(10 0) 3 30 120) dir log)
  ;; EL5-EL9: ellipses trimmed, extended and broken (ADR 0041), about
  ;; (0,0), semi-axes 5 and 3.
  (arq:command-case "EL5-trim-ellipse-whole" '(list (arq:ell doc '(0 0) '(5 0) 0.6 nil nil) (arq:ln doc '(4 -9) '(4 9))) '(command "_.TRIM" '(5 0) "") dir log)
  (arq:command-case "EL6-trim-ellipse-arc-middle" '(list (arq:ell doc '(0 0) '(5 0) 0.6 0.0 pi) (arq:ln doc '(0 0) '(0 9)) (arq:ln doc '(-4 0) '(-4 9))) '(command "_.TRIM" '(-2.5 2.598076211) "") dir log)
  (arq:command-case "EL7-extend-ellipse-arc" '(list (arq:ell doc '(0 0) '(5 0) 0.6 0.0 (/ pi 2)) (arq:ln doc '(-4 -9) '(-4 9))) '(arq:extend (cadr arq:o) '(0.2 2.997599)) dir log)
  (arq:command-case "EL8-break-ellipse-whole" '(list (arq:ell doc '(0 0) '(5 0) 0.6 nil nil)) '(command "_.BREAK" (arq:en (car arq:o)) "_F" '(5 0) '(0 3)) dir log)
  ;; EL10: the window crosses the ellipse and holds its center (a window
  ;; round the center alone selects nothing: crossing selects what it touches).
  (arq:command-case "EL10-stretch-ellipse-whole" '(list (arq:ell doc '(0 0) '(5 0) 0.6 nil nil)) '(command "_.STRETCH" "_C" '(6 1) '(-1 -1) "" '(0 0) '(2 3)) dir log)
  (arq:command-case "EL11-stretch-ellipse-arc-both-ends" '(list (arq:ell doc '(0 0) '(5 0) 0.6 0.0 (/ pi 2))) '(command "_.STRETCH" "_C" '(6 4) '(-1 -1) "" '(0 0) '(1 0)) dir log)
  (arq:command-case "EL12-stretch-ellipse-arc-one-end" '(list (arq:ell doc '(0 0) '(5 0) 0.6 0.0 (/ pi 2))) '(command "_.STRETCH" "_C" '(6 1) '(4 -1) "" '(0 0) '(1 0)) dir log)
  (arq:command-case "EL9-extend-line-to-ellipse" '(list (arq:ln doc '(-10 1.8) '(-8 1.8)) (arq:ell doc '(0 0) '(5 0) 0.6 nil nil)) '(arq:extend (cadr arq:o) '(-8.5 1.8)) dir log)
  ;; RECTANG, every option given each time (ProgeCAD remembers them)
  (arq:command-case "R1-rectang" 'nil '(arq:rectang 0.0 nil 0.0 0.0) dir log)
  (arq:command-case "R2-rectang-fillet" 'nil '(arq:rectang 1.0 nil 0.0 0.0) dir log)
  (arq:command-case "R3-rectang-chamfer" 'nil '(arq:rectang 0.0 '(1.0 2.0) 0.0 0.0) dir log)
  (arq:command-case "R4-rectang-width-elevation" 'nil '(arq:rectang 0.0 nil 0.5 2.0) dir log)
  (arq:rect-option '("_F" 0.0))
  (arq:rect-option '("_W" 0.0))
  (arq:rect-option '("_E" 0.0))
  (arq:rect-option '("_T" 0.0))
  (arq:restore)
  (arq:mark log "run done"))
