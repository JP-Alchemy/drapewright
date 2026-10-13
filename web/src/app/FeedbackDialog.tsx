// Feedback, from the header: an email started to Drapewright (in the
// person's own mail, saying which version and browser), or LinkedIn.

import { MessageSquareText } from 'lucide-react'
import { Button, Dialog, DialogTrigger, Heading, Modal, ModalOverlay } from 'react-aria-components'
import { t } from '../strings/en.ts'
import { LINKEDIN, aboutThisCopy, mailto } from './feedback.ts'

export function FeedbackDialog() {
  const text = t.feedback
  return (
    <DialogTrigger>
      <Button className="icon-button" aria-label={text.heading}>
        <MessageSquareText size={20} aria-hidden="true" />
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => (
              <>
                <Heading slot="title">{text.heading}</Heading>
                <p>{text.intro}</p>
                <div className="feedback-links">
                  <a
                    className="button primary"
                    href={mailto(text.subject, text.body(aboutThisCopy()))}
                  >
                    {text.email}
                  </a>
                  <a className="button" href={LINKEDIN} target="_blank" rel="noopener noreferrer">
                    {text.linkedin}
                  </a>
                </div>
                <div className="dialog-actions">
                  <Button className="button" onPress={close}>
                    {t.actions.close}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}
