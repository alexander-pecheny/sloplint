<?php
function ladder($x, $y) {
    if ($x == 1) { return "a"; }
    elseif ($x == 2) { return "b"; }
    elseif ($x == 3 && $y) { return "c"; }
    elseif ($x == 4) { return "d"; }
    elseif ($x == 5) { return "e"; }
    else { return "f"; }
}
