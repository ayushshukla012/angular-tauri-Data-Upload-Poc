package com.insight.upload.service;

import com.insight.upload.dto.PacketResponse;
import com.insight.upload.dto.SubmitPacketRequest;
import com.insight.upload.entity.Packet;
import com.insight.upload.exception.PacketNotFoundException;
import com.insight.upload.repository.PacketRepository;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.Optional;

@Service
public class PacketService {

    private final PacketRepository packetRepository;

    public PacketService(PacketRepository packetRepository) {
        this.packetRepository = packetRepository;
    }

    /**
     * Idempotent, same reasoning as CaseService.submitCase: the client's offline-reconnect retry
     * may resend the same batchNumber, which is a replay, not a conflict.
     */
    @Transactional
    public PacketResponse submitPacket(SubmitPacketRequest request) {
        Optional<Packet> existing = packetRepository.findById(request.batchNumber());
        if (existing.isPresent()) {
            return toResponse(existing.get());
        }

        Packet packet = new Packet(request.batchNumber(), request.description(), request.submittingPersonName(),
                request.submittingPersonAddress(), request.submittingPersonMobile(), request.submittingPersonEmail());
        packetRepository.save(packet);
        return toResponse(packet);
    }

    @Transactional(readOnly = true)
    public PacketResponse getPacket(String batchNumber) {
        return toResponse(requirePacket(batchNumber));
    }

    Packet requirePacket(String batchNumber) {
        return packetRepository.findById(batchNumber).orElseThrow(() -> new PacketNotFoundException(batchNumber));
    }

    private PacketResponse toResponse(Packet packet) {
        return new PacketResponse(
                packet.getBatchNumber(),
                packet.getDescription(),
                packet.getSubmittingPersonName(),
                packet.getSubmittingPersonAddress(),
                packet.getSubmittingPersonMobile(),
                packet.getSubmittingPersonEmail(),
                packet.getApprovalStatus().name(),
                packet.getCreatedAt());
    }
}
