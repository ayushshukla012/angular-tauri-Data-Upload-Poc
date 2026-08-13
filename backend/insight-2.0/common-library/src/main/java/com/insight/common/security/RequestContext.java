package com.insight.common.security;

public record RequestContext(String correlationId, String principal) {

    private static final ThreadLocal<RequestContext> CURRENT = new ThreadLocal<>();

    public static void set(RequestContext context) {
        CURRENT.set(context);
    }

    public static RequestContext current() {
        return CURRENT.get();
    }

    public static void clear() {
        CURRENT.remove();
    }
}
