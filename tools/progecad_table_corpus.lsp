;;; Original native-table fixture and observation helpers for milestone 03.4.
;;; Load inside ProgeCAD; this is not a product API or a codec-generated fixture.
(vl-load-com)

(defun arq:measure-value (value)
  (cond ((= (type value) 'REAL) (rtos value 2 10))
        ((= (type value) 'INT) (itoa value))
        (T (vl-prin1-to-string value))))

(defun arq:emit (file key value)
  (write-line (strcat key (chr 9) (arq:measure-value value)) file))

(defun arq:raw-measures (file prefix obj / raw item row cell values)
  (setq raw (entget (vlax-vla-object->ename obj)) row 0 cell -1 values nil)
  (foreach item raw
    (cond
      ((and (= cell -1) (= (car item) 141))
       (arq:emit file (strcat prefix "row_height/" (itoa row)) (cdr item))
       (setq row (1+ row)))
      ((= (car item) 171)
       (if values (arq:emit file (strcat prefix "cell_dxf/" (itoa cell)) (reverse values)))
       (setq cell (1+ cell) values (list item)))
      ((and (>= cell 0) (member (car item)
          '(172 173 174 175 176 178 145 140 7 170 91 63 64 65 66 68 69
            273 274 275 276 277 278 279 283 284 285 286 287 288 289
            1 2 3 90 93 94 95 96 97 98 99 301 300 302 304 40 41)))
       (setq values (cons item values)))))
  (if values (arq:emit file (strcat prefix "cell_dxf/" (itoa cell)) (reverse values))))

(defun arq:audit-doc (path doc / file obj pt prefix row col rows columns)
  (if (findfile path) (vl-exit-with-error "Observation exists; choose a new path"))
  (setq file (open path "w"))
  (arq:emit file "schema" "progecad-table-v1")
  (arq:emit file "drawing" (vla-get-Name doc))
  (vlax-for obj (vla-get-ModelSpace doc)
    (if (= (vla-get-ObjectName obj) "AcDbTable")
      (progn
        (setq pt (vlax-safearray->list (vlax-variant-value (vla-get-InsertionPoint obj)))
              prefix (strcat "table/" (rtos (car pt) 2 0) "/")
              rows (vla-get-Rows obj) columns (vla-get-Columns obj))
        (arq:emit file (strcat prefix "entity_dxf") (entget (vlax-vla-object->ename obj)))
        (arq:raw-measures file prefix obj)
        (arq:emit file (strcat prefix "rows") rows)
        (arq:emit file (strcat prefix "columns") columns)
        (arq:emit file (strcat prefix "height") (vla-get-Height obj))
        (arq:emit file (strcat prefix "horizontal_margin") (vla-get-HorzCellMargin obj))
        (arq:emit file (strcat prefix "vertical_margin") (vla-get-VertCellMargin obj))
        (arq:emit file (strcat prefix "style") (vla-get-StyleName obj))
        (foreach kind '(1 2 4)
          (arq:emit file (strcat prefix "text_height/" (itoa kind)) (vla-GetTextHeight obj kind)))
        (setq col 0)
        (repeat columns
          (arq:emit file (strcat prefix "column_width/" (itoa col)) (vla-GetColumnWidth obj col))
          (setq col (1+ col)))
        (setq row 0)
        (repeat rows
          (setq col 0)
          (repeat columns
            (arq:emit file (strcat prefix "text/" (itoa row) "/" (itoa col)) (vla-GetText obj row col))
            (arq:emit file (strcat prefix "cell_state/" (itoa row) "/" (itoa col)) (vla-GetCellState obj row col))
            (setq col (1+ col)))
          (setq row (1+ row))))))
  (close file)
  (princ))

(defun arq:audit (path)
  (arq:audit-doc path (vla-get-ActiveDocument (vlax-get-acad-object))))

;;; Load this file again after opening another drawing: Lisp functions are document-local.
(defun arq:create-tables (/ doc space obj spec row col)
  (setq doc (vla-get-ActiveDocument (vlax-get-acad-object))
        space (vla-get-ModelSpace doc))
  (if (/= (vla-get-Count space) 0) (vl-exit-with-error "Use an empty new drawing"))
  (foreach spec '((0.0 1.5 1.5 2.5 "BASE")
                 (160.0 4.5 3.0 3.0 "MARGINS")
                 (320.0 1.5 1.5 2.5 "MERGED")
                 (480.0 2.0 2.0 4.0 "BORDERS"))
    (setq obj (vla-AddTable space (vlax-3d-point (list (car spec) 0.0 0.0)) 5 3 14.0 40.0))
    (vla-put-HorzCellMargin obj (cadr spec))
    (vla-put-VertCellMargin obj (caddr spec))
    (foreach kind '(1 2 4) (vla-SetTextHeight obj kind (nth 3 spec)))
    (setq row 0)
    (repeat 5
      (setq col 0)
      (repeat 3
        (vla-SetText obj row col (strcat (nth 4 spec) " R" (itoa row) " C" (itoa col)))
        (setq col (1+ col)))
      (setq row (1+ row)))
    (if (= (car spec) 160.0)
      (progn (vla-SetColumnWidth obj 0 25.0) (vla-SetColumnWidth obj 1 55.0)
             (vla-SetRowHeight obj 3 20.0)))
    (vla-Update obj))
  (vla-Regen doc 1)
  (vla-ZoomExtents (vlax-get-acad-object))
  (princ))
(princ)
