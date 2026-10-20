<?php
// Regenerates rust/locales/<locale>.json from lang/<locale>/{validation,passwords,auth}.php,
// the Laravel translation arrays the server needs (validation messages, password
// reset and login messages). The app strings in lang/<locale>.json are embedded
// directly. Usage: php rust/locales/generate.php

$root = dirname(__DIR__, 2);
foreach (glob("$root/lang/*.json") as $json) {
    $locale = basename($json, '.json');
    $out = [];
    foreach (['validation', 'passwords', 'auth'] as $group) {
        $file = "$root/lang/$locale/$group.php";
        $out[$group] = file_exists($file) ? require $file : new stdClass;
    }
    file_put_contents(
        __DIR__."/$locale.json",
        json_encode($out, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_PRETTY_PRINT)."\n"
    );
}
