package com.camarelol.catalog;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Versioned JSON-RPC server: one JSON object per line (NDJSON).
 *
 * The protocol is the same one the Rust core speaks (docs/IPC-PROTOCOL.md);
 * this component only answers read-only and advisory methods. A write request
 * is always refused here — the catalog can never order a write to disk.
 */
public final class JsonRpcServer implements AutoCloseable {

    public record Request(String id, int protocolVersion, String method,
                         Map<String, Object> params, Integer timeoutMs,
                         String cancel) {
    }

    private final int port;
    private volatile boolean running;

    public JsonRpcServer(int port) {
        this.port = port;
    }

    public static void main(String[] args) throws IOException {
        int port = args.length > 0 ? Integer.parseInt(args[0]) : 47812;
        try (JsonRpcServer server = new JsonRpcServer(port)) {
            server.start();
            System.out.println("camareLOL catalog listening on port " + port);
            Thread.currentThread().join(); // run until killed
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    public void start() throws IOException {
        running = true;
        try (ServerSocket serverSocket = new ServerSocket(port)) {
            serverSocket.setReuseAddress(true);
            while (running) {
                Socket client = serverSocket.accept();
                Thread handler = new Thread(() -> handle(client));
                handler.setDaemon(true);
                handler.start();
            }
        }
    }

    private void handle(Socket client) {
        try (Socket socket = client;
             BufferedReader in = new BufferedReader(
                     new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
             PrintWriter out = new PrintWriter(new OutputStreamWriter(
                     socket.getOutputStream(), StandardCharsets.UTF_8), true)) {
            String line;
            while ((line = in.readLine()) != null) {
                Map<String, Object> response = dispatch(line.trim());
                if (response != null) {
                    out.println(Json.write(response));
                }
            }
        } catch (IOException ignored) {
            // client went away
        }
    }

    Map<String, Object> dispatch(String line) {
        Map<String, Object> response = new LinkedHashMap<>();
        if (line.isEmpty()) {
            response.put("status", "error");
            response.put("error", error("parse_failed", "empty request"));
            return response;
        }
        try {
            Map<String, Object> request = Json.parseObject(line);
            String id = request.containsKey("id")
                    ? String.valueOf(request.get("id"))
                    : UUID.randomUUID().toString();
            int protocolVersion = request.containsKey("protocol_version")
                    ? Json.asInt(request.get("protocol_version"))
                    : 0;
            String method = Json.asString(request.get("method"));
            @SuppressWarnings("unchecked")
            Map<String, Object> params = request.containsKey("params")
                    ? (Map<String, Object>) request.get("params")
                    : new LinkedHashMap<>();

            response.put("id", id);
            response.put("protocol_version", 1);
            if (protocolVersion != 1) {
                response.put("status", "error");
                response.put("error", error("unsupported_schema",
                        "this service speaks protocol version 1"));
                return response;
            }

            switch (method) {
                case "hello" -> {
                    Map<String, Object> result = new LinkedHashMap<>();
                    result.put("service", "camareLOL-catalog");
                    result.put("version", "0.1.0");
                    response.put("status", "ok");
                    response.put("result", result);
                }
                case "list_groups" -> {
                    List<Map<String, Object>> groups = new ArrayList<>();
                    for (PackageCatalog.PackageGroup group : PackageCatalog.groups()) {
                        Map<String, Object> g = new LinkedHashMap<>();
                        g.put("id", group.id());
                        g.put("title", group.title());
                        List<Map<String, String>> entries = new ArrayList<>();
                        for (PackageCatalog.PackageEntry e : group.entries()) {
                            Map<String, String> entry = new LinkedHashMap<>();
                            entry.put("name", e.name());
                            entry.put("description", e.description());
                            entry.put("source", e.source().name());
                            entries.add(entry);
                        }
                        g.put("packages", entries);
                        groups.add(g);
                    }
                    Map<String, Object> result = new LinkedHashMap<>();
                    result.put("groups", groups);
                    response.put("status", "ok");
                    response.put("result", result);
                }
                case "resolve_profile" -> {
                    String desktop = params.containsKey("desktop")
                            ? Json.asString(params.get("desktop")) : "console";
                    String server = params.containsKey("display_server")
                            ? Json.asString(params.get("display_server")) : "auto";
                    String dm = params.containsKey("display_manager")
                            ? Json.asString(params.get("display_manager")) : null;
                    boolean aur = params.containsKey("aur_enabled")
                            && Json.asBool(params.get("aur_enabled"));
                    List<Object> extra = params.containsKey("extra_packages")
                            ? Json.asArray(params.get("extra_packages"))
                            : List.of();
                    try {
                        ProfileService.ProfilePlan plan = ProfileService.resolve(
                                desktop, server, dm, aur, extra.toArray(new String[0]));
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("desktop", plan.desktop());
                        result.put("display_server", plan.displayServer());
                        if (plan.displayManager() != null) {
                            result.put("display_manager", plan.displayManager());
                        }
                        result.put("packages", List.of(plan.officialPackages()));
                        result.put("warnings", List.of(plan.warnings()));
                        response.put("status", "ok");
                        response.put("result", result);
                    } catch (IllegalArgumentException e) {
                        response.put("status", "error");
                        response.put("error", error("invalid_config", e.getMessage()));
                    }
                }
                // every write-type method is refused here, by design
                case "apply_plan", "format_partitions", "write_partition_table" -> {
                    response.put("status", "error");
                    response.put("error", error("not_supported",
                            "the catalog never performs destructive operations"));
                }
                default -> {
                    response.put("status", "error");
                    response.put("error", error("parse_failed", "unknown method: " + method));
                }
            }
            return response;
        } catch (Exception e) {
            response.put("status", "error");
            response.put("error", error("internal", String.valueOf(e)));
            return response;
        }
    }

    private static Map<String, Object> error(String code, String message) {
        Map<String, Object> err = new LinkedHashMap<>();
        err.put("code", code);
        err.put("message", message);
        return err;
    }

    @Override
    public void close() {
        running = false;
    }
}
