<?php
// Loads routes/web.php and routes/api.php with a stub Route facade and prints
// every route as JSON lines. Usage: php docs/rewrite/tools/routes.php

namespace Illuminate\Support\Facades {
    class RouteStub
    {
        public static array $stack = [];
        public static array $routes = [];
        public array $attrs = ['prefix' => [], 'name' => [], 'middleware' => [], 'without' => []];
        public ?array $route = null;

        public function prefix($p) { $this->attrs['prefix'][] = trim($p, '/'); return $this; }
        public function name($n) {
            if ($this->route !== null) { self::$routes[$this->route['i']]['name'] .= $n; return $this; }
            $this->attrs['name'][] = $n; return $this;
        }
        public function middleware($m) {
            $m = is_array($m) ? $m : [$m];
            if ($this->route !== null) { self::$routes[$this->route['i']]['middleware'] = array_merge(self::$routes[$this->route['i']]['middleware'], $m); return $this; }
            $this->attrs['middleware'] = array_merge($this->attrs['middleware'], $m); return $this;
        }
        public function withoutMiddleware($m) { return $this; }
        public function where(...$a) { return $this; }
        public function only($a) {
            $i = $this->route['res'];
            foreach (self::$routes as $k => $r) {
                if (($r['res'] ?? null) === $i && ! in_array($r['action'], $a)) unset(self::$routes[$k]);
            }
            return $this;
        }
        public function group($cb) {
            self::$stack[] = $this->attrs;
            $cb();
            array_pop(self::$stack);
            return $this;
        }
        public static function ctx(): array {
            $prefix = []; $name = ''; $mw = [];
            foreach (self::$stack as $a) {
                $prefix = array_merge($prefix, array_filter($a['prefix'], fn ($p) => $p !== ''));
                $name .= implode('', $a['name']);
                $mw = array_merge($mw, $a['middleware']);
            }
            return [$prefix, $name, $mw];
        }
        public static function add($methods, $uri, $action, $res = null): self {
            [$prefix, $name, $mw] = self::ctx();
            $uri = trim($uri, '/');
            $full = '/'.implode('/', array_filter(array_merge($prefix, [$uri]), fn ($p) => $p !== ''));
            if (is_array($action)) { [$c, $m] = $action; } elseif (is_string($action)) { $c = $action; $m = '__invoke'; } else { $c = 'Closure'; $m = ''; }
            self::$routes[] = ['methods' => $methods, 'uri' => $full, 'name' => $name, 'controller' => $c, 'action' => $m, 'middleware' => $mw, 'res' => $res];
            $s = new self; $s->route = ['i' => array_key_last(self::$routes), 'res' => $res];
            return $s;
        }
    }

    class Route
    {
        public static function __callStatic($fn, $args)
        {
            $s = new RouteStub;
            switch ($fn) {
                case 'get': return RouteStub::add(['GET'], $args[0], $args[1]);
                case 'post': return RouteStub::add(['POST'], $args[0], $args[1]);
                case 'put': return RouteStub::add(['PUT'], $args[0], $args[1]);
                case 'patch': return RouteStub::add(['PATCH'], $args[0], $args[1]);
                case 'delete': return RouteStub::add(['DELETE'], $args[0], $args[1]);
                case 'permanentRedirect': return RouteStub::add(['GET'], $args[0], 'Redirect:'.$args[1]);
                case 'apiResource':
                    [$path, $c] = $args; $res = uniqid();
                    $p = trim($path, '/'); $one = rtrim($p, 's');
                    RouteStub::add(['GET'], $p, [$c, 'index'], $res)->name("$p.index");
                    RouteStub::add(['POST'], $p, [$c, 'store'], $res)->name("$p.store");
                    RouteStub::add(['GET'], "$p/{{$one}}", [$c, 'show'], $res)->name("$p.show");
                    RouteStub::add(['PUT', 'PATCH'], "$p/{{$one}}", [$c, 'update'], $res)->name("$p.update");
                    $last = RouteStub::add(['DELETE'], "$p/{{$one}}", [$c, 'destroy'], $res)->name("$p.destroy");
                    return $last;
                default: return $s->$fn(...$args);
            }
        }
    }

    class Auth { public static function __callStatic($f, $a) { return null; } }
    class Redirect { public static function __callStatic($f, $a) { return null; } }
}

namespace {
    function config($k) { return ['jetstream.auth_session' => 'auth_session', 'services.telegram-bot-api.token' => 'x', 'services.telegram-bot-api.webhook' => '{webhook}'][$k] ?? null; }
    function route(...$a) { return ''; }

    $root = dirname(__DIR__, 3);
    $out = [];
    foreach (['web', 'api'] as $file) {
        \Illuminate\Support\Facades\RouteStub::$routes = [];
        \Illuminate\Support\Facades\RouteStub::$stack = $file === 'api' ? [['prefix' => ['api'], 'name' => [], 'middleware' => ['api'], 'without' => []]] : [];
        require $root."/routes/$file.php";
        foreach (\Illuminate\Support\Facades\RouteStub::$routes as $r) {
            unset($r['res']);
            $r['file'] = $file;
            echo json_encode($r, JSON_UNESCAPED_SLASHES), "\n";
        }
    }
}
