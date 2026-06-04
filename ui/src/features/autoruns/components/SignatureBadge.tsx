import { Badge } from "@/components/ui/badge";
import type { SignatureStatus } from "../types";

interface Props {
  status: SignatureStatus;
}

export function SignatureBadge({ status }: Props) {
  switch (status.kind) {
    case "valid":
      return <Badge variant="success">已签名</Badge>;
    case "invalid":
      return <Badge variant="danger">签名异常</Badge>;
    case "unsigned":
      return <Badge variant="warning">未签名</Badge>;
    case "not_verified":
      return <Badge variant="default">待验证</Badge>;
  }
}
