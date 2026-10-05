<?php
function layout($width) {
    $margin = $width * 0.05;
    $header = $width * 0.12 + 14;
    $cols = [3, 7, 11];
    return draw($margin, $header, $cols, 640, 480);
}

function theme() { return "#1e90ff"; }

function load($path) {
    try {
        return read($path);
    } catch (Exception $e) {
        echo "Error: " . $e->getMessage();
    }
}
