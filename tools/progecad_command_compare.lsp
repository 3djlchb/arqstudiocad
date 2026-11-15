;;; Behavioral comparison for milestone 02.14 in ProgeCAD (docs/testing/
;;; stretch-arcs-windows.md, break-closed-polylines-windows.md,
;;; array-rectangular-windows.md):
;;; synthetic entities made here, stretched or broken by ProgeCAD's own
;;; commands at fixed points, every field written to a log, each case saved
;;; before and after as DWG 2018 and DXF 2018. Load inside ProgeCAD on a
;;; blank drawing from its own template; not a product API.
;;;
;;;   (load "tools/progecad_command_compare.lsp")
;;;   (arq:compare-02-14 "C:/path/to/saved" "C:/path/to/cases.log")
;;;   (arq:compare-array "C:/path/to/array" "C:/path/to/array.log")
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
  (strcat d " layer=" (vla-get-Layer obj) " color=" (itoa (vla-get-Color obj))))

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
