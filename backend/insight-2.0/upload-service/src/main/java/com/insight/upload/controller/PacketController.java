package com.insight.upload.controller;

import com.insight.upload.dto.PacketResponse;
import com.insight.upload.dto.SubmitPacketRequest;
import com.insight.upload.service.PacketService;
import jakarta.validation.Valid;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/v1/packets")
public class PacketController {

    private final PacketService packetService;

    public PacketController(PacketService packetService) {
        this.packetService = packetService;
    }

    /**
     * Submits a packet's (batch's) details — the legacy app's "Packet Details" screen, minus any
     * XML/package generation. Idempotent, same as {@code POST /api/v1/cases}.
     */
    @PostMapping
    public ResponseEntity<PacketResponse> submitPacket(@Valid @RequestBody SubmitPacketRequest request) {
        PacketResponse response = packetService.submitPacket(request);
        return ResponseEntity.status(HttpStatus.CREATED).body(response);
    }

    @GetMapping("/{batchNumber}")
    public ResponseEntity<PacketResponse> getPacket(@PathVariable String batchNumber) {
        return ResponseEntity.ok(packetService.getPacket(batchNumber));
    }
}
